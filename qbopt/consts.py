"""
Which values are known, and what they are.

Ordinary sparse constant propagation over the SSA graph, with one thing
about this machine that is not ordinary and cannot be skipped: a value here
is rooted to its 32-bit parent, and almost every instruction BC emits writes
sixteen bits of it. `mov ax,5` does not make eax five. It makes the low half
five and leaves the high half whatever the pass that widened it put there.

So a fact is a width as well as a number -- "the low `width` bytes of this
value are `n`" -- and an operation only folds where the widths agree. Taking
the number alone would be wrong in the direction that matters: it would fold
a 32-bit use of a value only half of which is known, and produce a plausible
answer that is not the program's.

What seeds it, measured across fixtures/omf and bench/nbody.bas: 2,120
`mov reg,imm16`, 155 `mov reg,imm32`, and 56 arithmetic instructions against
an immediate. What it will not seed from is memory. A static's contents are
constant far more often than not -- BC initialises one and never writes it
again -- but proving that needs to know every store in the module reaches
it, which is memory.py's question and a whole-program one, not this pass's.

Meets are not needed yet and so are not written. A phi whose arguments are
all the same constant is one, and nothing here folds it: with 871 branches
all reading a comparison, the conditional half of a sparse conditional
propagation is where that value would come from, and it is a separate piece
of work with its own correctness argument.
"""

from dataclasses import dataclass

from iced_x86 import Register_

from qbopt import ir
from qbopt import mir
from qbopt import runtime

# What each operation does to two known numbers, within one width. Division
# is absent deliberately: BC's own divide has semantics this pass already
# refuses to reproduce (calls.py -- `x/0` and the signed extreme), and a
# folder that answered them here would be inventing a result the running
# program never produces.
ARITH = {
    "add": lambda a, b: a + b,
    "sub": lambda a, b: a - b,
    "and": lambda a, b: a & b,
    "or": lambda a, b: a | b,
    "xor": lambda a, b: a ^ b,
    "shl": lambda a, b: a << (b & 31),
    "imul": lambda a, b: a * b,
}

UNARY = {
    "neg": lambda a: -a,
    "not": lambda a: ~a,
    "inc": lambda a: a + 1,
    "dec": lambda a: a - 1,
}


# What a cell holds: keyed on its address and width, because two
# widths at one address are two different facts.
Cells = dict


@dataclass(frozen=True, slots=True)
class Known:
    """The low `width` bytes of a value are `n`. Nothing is said above them."""

    n: int
    width: int

    def __repr__(self) -> str:
        return f"{self.n:#x}:{self.width}"


def masked(n: int, width: int) -> int:
    return n & ((1 << (width * 8)) - 1)


def _put(op: mir.Op, known: dict[mir.Value, Known]) -> Known | None:
    """What this store puts in the cell, where that is a number.

    Two shapes and both are common: BC writes `mov word [n],7` for an
    initialiser, so the number is an immediate the instruction carries and
    not a value at all, and it writes `mov [s],ax` for an assignment, where
    the number is whatever ax was known to hold.
    """
    what = op.made if op.made is not None else (op.node.semantics if op.node is not None else None)
    if what is None or what.op is not ir.Operation.MOVE:
        return None
    for one in what.sources:
        if isinstance(one, ir.Imm):
            return Known(masked(one.value, one.width), one.width)
    from_value = [one for one in op.uses if one in known and not one.flags]
    return known[from_value[0]] if len(from_value) == 1 else None


def _kills(here: Cells, op: mir.Op, known: dict[mir.Value, Known], dgroup: frozenset[int], calls: dict[int, str]) -> Cells:
    """The cell facts still standing after this operation."""
    if op.at in calls:
        contract = runtime.contract(calls[op.at])
        if runtime.writes_caller_memory(contract) or runtime.barrier(contract):
            return {}
    for ref in op.stores:
        if ref.addr is None:
            return {}  # a store nothing can name reaches every cell
        here = {
            where: fact
            for where, fact in here.items()
            if not mir.overlapping(mir.MemRef(where[0], where[1], None, None), ref, dgroup)
        }
        put = _put(op, known)
        if put is not None:
            here[(ref.addr, ref.width)] = put
    return here


def cells(
    body: mir.MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    known: dict[mir.Value, Known] | None = None,
) -> dict[tuple[int, int], Cells]:
    """What each memory cell holds before each operation, where it is a number.

    Forward to a fixed point, meeting at a join on agreement, which is the
    same shape as known() and for the same reason. Keyed on the block and
    the operation's index within it rather than its address, because
    absorption puts several operations on one address.

    A block none of whose predecessors have been visited yet is *deferred*,
    not treated as knowing nothing. Saying "nothing is known here" poisons
    the meet for good -- a loop body's only predecessor is its header, which
    is unvisited on the first round, so hotlop could never learn that `n` is
    7 even though the entry block says so three instructions earlier.
    """
    known = known if known is not None else {}
    outof: dict[int, Cells | None] = {block.at: None for block in body.blocks}
    preds = {
        block.at: [one.at for one in body.blocks if block.at in one.succ] for block in body.blocks
    }

    def entering(at: int) -> Cells | None:
        if not preds[at]:
            return {}
        seen = [outof[one] for one in preds[at] if outof[one] is not None]
        if not seen:
            return None
        return {
            where: fact
            for where, fact in seen[0].items()
            if all(one.get(where) == fact for one in seen[1:])
        }

    changing = True
    while changing:
        changing = False
        for block in body.blocks:
            here = entering(block.at)
            if here is None:
                continue
            for op in block.ops:
                here = _kills(here, op, known, dgroup, calls)
            if outof[block.at] != here:
                outof[block.at] = here
                changing = True

    found: dict[tuple[int, int], Cells] = {}
    for block in body.blocks:
        here = entering(block.at) or {}
        for index, op in enumerate(block.ops):
            found[(block.at, index)] = here
            here = _kills(here, op, known, dgroup, calls)
    return found


def _source_value(op: mir.Op, register: Register_, origin: dict[mir.Value, Register_]) -> mir.Value | None:
    """The SSA value standing for this semantic register operand."""
    root = ir.ROOT.get(register, register)
    return next((one for one in op.uses if origin.get(one) is root), None)


def _operand(
    op: mir.Op,
    where: ir.Loc,
    known: dict[mir.Value, Known],
    origin: dict[mir.Value, Register_],
    here: Cells | None = None,
) -> Known | None:
    """One semantic operand as a number, if it is one."""
    match where:
        case ir.Imm(value=value, width=width):
            return Known(masked(value, width), width)
        case ir.Reg(register=register, width=width):
            value = _source_value(op, register, origin)
            fact = known.get(value) if value is not None else None
            return fact if fact is not None and fact.width >= width else None
        case ir.Mem(addr=addr, width=width) if here is not None and addr is not None:
            # A cell whose content is known is as good as an immediate.
            # Without this the propagation stops at BC's first store: it
            # keeps every variable in memory, so `n * k` reads two cells and
            # neither is a value this could ask about.
            fact = here.get((addr, width))
            return fact if fact is not None and fact.width >= width else None
        case _:
            return None  # an address, or a cell nothing has said anything about


def _defined(op: mir.Op, semantics=None, origin: dict | None = None) -> mir.Value | None:
    """The value this op's first destination gets, flags aside.

    Nearly every arithmetic instruction on this machine defines its result
    and the flags together, so asking for a single definition rejects all of
    them -- which it did, and the propagation found nothing but its own
    seeds until the flags were excluded here.

    Two results are the other case, and refusing them cost more: a widening
    `imul` defines dx:ax, and hotlop's `n * k` is exactly that. Both halves
    are constant and the high one is dead, and nothing here could say so, so
    the product was recomputed on all twenty passes of the loop. The result
    the fold is about is the one the *first* destination names; which value
    that is comes from `origin`, the same way a source operand is resolved.
    """
    real = [one for one in op.defines if not one.flags]
    if len(real) == 1:
        return real[0]
    if not real or semantics is None or origin is None or not semantics.dests:
        return None
    first = semantics.dests[0]
    if not isinstance(first, ir.Reg):
        return None
    root = ir.ROOT.get(first.register, first.register)
    return next((one for one in real if origin.get(one) is root), None)


def _result(
    op: mir.Op,
    known: dict[mir.Value, Known],
    origin: dict[mir.Value, Register_],
    here: Cells | None = None,
) -> Known | None:
    """What this operation computes, where every input is known."""
    # What a transform decided this op computes, where it decided; the
    # node's own otherwise. An op rewritten by an earlier pass is raised
    # again before this looks, so its `made` is the only account of it.
    semantics = op.made if op.made is not None else (op.node.semantics if op.node is not None else None)
    if semantics is None or not ir.modelled(semantics) or _defined(op, semantics, origin) is None:
        return None

    parts: list[Known] = []
    for one in semantics.sources:
        got = _operand(op, one, known, origin, here)
        if got is None:
            return None
        parts.append(got)
    if not parts:
        return None
    width = min(one.width for one in parts)

    match semantics.op:
        case ir.Operation.MOVE if len(parts) == 1:
            return Known(masked(parts[0].n, width), width)
        case ir.Operation.BINARY | ir.Operation.MULTIPLY if len(parts) == 2 and semantics.name in ARITH:
            a, b = parts
            return Known(masked(ARITH[semantics.name](a.n, b.n), width), width)
        case ir.Operation.UNARY if len(parts) == 1 and semantics.name in UNARY:
            return Known(masked(UNARY[semantics.name](parts[0].n), width), width)
        case _:
            return None


def known(
    body: mir.MirBody,
    dgroup: frozenset[int] | None = None,
    calls: dict[int, str] | None = None,
) -> dict[mir.Value, Known]:
    """Every value this body computes that is a number, to a fixed point.

    Forward over the blocks until nothing new is learned. A value's fact
    only ever goes from unknown to known and never changes once set --
    which is SSA's own doing, since the value is defined once -- so the
    walk terminates on the count of values rather than on any ordering.
    """
    facts: dict[mir.Value, Known] = {}
    held: dict[tuple[int, int], Cells] = {}
    changing = True
    while changing:
        changing = False
        # What memory holds, recomputed from what is known so far. The two
        # feed each other: a cell is known because a value was stored to it,
        # and a value is known because it was read from a cell. Running them
        # to one fixed point together is what lets `n = 7 : k = 3` reach the
        # `n * k` inside the loop, which is three statements and a store
        # away.
        if dgroup is not None and calls is not None:
            held = cells(body, dgroup, calls, facts)
        for block in body.blocks:
            # A join is known where every path into it agrees. Nothing else
            # about a phi is knowable -- and this is what makes the
            # propagation cross-block rather than merely whole-body: a value
            # defined in one branch and read after the join was invisible
            # until the phi carrying it could be a number too.
            for phi in block.phis:
                if phi.result in facts or not phi.incoming:
                    continue
                seen = [facts.get(one) for one in phi.incoming.values()]
                known = [one for one in seen if one is not None]
                if len(known) != len(seen) or len({(o.n, o.width) for o in known}) != 1:
                    continue
                facts[phi.result] = known[0]
                changing = True
            for index, op in enumerate(block.ops):
                semantics = op.made if op.made is not None else (op.node.semantics if op.node is not None else None)
                target = _defined(op, semantics, body.origin)
                if target is None or target in facts:
                    continue
                found = _result(op, facts, body.origin, held.get((block.at, index)))
                if found is not None:
                    facts[target] = found
                    changing = True
    return facts
