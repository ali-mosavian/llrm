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

from qbopt import ir
from qbopt import mir
from qbopt import runtime

# What each operation does to two known numbers, within one width. Division
# is absent deliberately: BC's own divide has semantics this pass already
# refuses to reproduce (calls.py -- `x/0` and the signed extreme), and a
# folder that answered them here would be inventing a result the running
# program never produces.
ARITH = {
    mir.Kind.ADD: lambda a, b: a + b,
    mir.Kind.SUB: lambda a, b: a - b,
    mir.Kind.AND: lambda a, b: a & b,
    mir.Kind.OR: lambda a, b: a | b,
    mir.Kind.XOR: lambda a, b: a ^ b,
    mir.Kind.SHL: lambda a, b: a << (b & 31),
    mir.Kind.SHR: lambda a, b: (a & 0xFFFFFFFF) >> (b & 31),
    mir.Kind.MUL: lambda a, b: a * b,
}

UNARY = {
    mir.Kind.NEG: lambda a: -a,
    mir.Kind.NOT: lambda a: ~a,
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

    Two shapes and both are common: BC writes an initialiser as a store of
    a constant, so the number is in the operation itself and is no value at
    all, and it writes an assignment as a store of a value, where the
    number is whatever that value was known to hold.
    """
    if op.kind is not mir.Kind.STORE:
        return None
    for one in op.args:
        if isinstance(one, mir.Const):
            return Known(masked(one.n, one.width), one.width)
    from_value = [one for one in op.uses if one in known and not one.flags]
    return known[from_value[0]] if len(from_value) == 1 else None


def _kills(
    here: Cells, op: mir.Op, known: dict[mir.Value, Known], dgroup: frozenset[int], calls: dict[int, str]
) -> Cells:
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
    preds = {block.at: [one.at for one in body.blocks if block.at in one.succ] for block in body.blocks}

    def entering(at: int) -> Cells | None:
        if not preds[at]:
            return {}
        seen = [outof[one] for one in preds[at] if outof[one] is not None]
        if not seen:
            return None
        return {where: fact for where, fact in seen[0].items() if all(one.get(where) == fact for one in seen[1:])}

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


def _read(fact: Known | None, width: int) -> Known | None:
    if fact is None or fact.width < width:
        return None
    return Known(masked(fact.n, width), width)


def _operand(op: mir.Op, one: mir.Arg, known: dict, here: Cells | None = None) -> Known | None:
    """One operand as a number, if it is one.

    MIR's own operands: a constant is one, a value is one where something
    has said so, and a cell is one where the memory walk has. This matched
    ir.Reg and resolved it back to a value through `origin` -- a pass
    asking which register an operand named.
    """
    if isinstance(one, mir.Const):
        return Known(masked(one.n, one.width), one.width)
    if isinstance(one, mir.Held):
        return _read(known.get(one.value), one.width)
    if isinstance(one, mir.Cell) and here is not None and one.ref.addr is not None:
        # A cell whose content is known is as good as a constant. Without
        # this the propagation stops at BC's first store: it keeps every
        # variable in memory, so `n * k` reads two cells and neither is a
        # value this could ask about.
        return _read(here.get((one.ref.addr, one.ref.width)), one.ref.width)
    return None


def _defined(op: mir.Op, semantics: ir.Semantics | None = None, origin: dict | None = None) -> mir.Value | None:
    """The value this operation's first result gets, flags aside.

    Nearly every arithmetic operation defines its result and the flags
    together, so asking for a single definition rejects all of them --
    which it did, and the propagation found nothing but its own seeds until
    the flags were excluded here.

    Two results are the other case, and refusing them cost more: a widening
    multiply defines a pair, and hotlop's `n * k` is exactly that. Both
    halves are constant and the high one is dead, and nothing here could
    say so, so the product was recomputed on all twenty passes of the loop.
    The result the fold is about is the one the first result names -- which
    the operation says itself now, where it used to be looked up by which
    register the destination was.
    """
    real = [one for one in op.defines if not one.flags]
    if len(real) == 1:
        return real[0]
    first = next((one for one in op.results if isinstance(one, mir.Held)), None)
    if first is None:
        return None
    return first.value if first.value in real else None


def _result(
    op: mir.Op,
    known: dict[mir.Value, Known],
    origin: dict | None = None,
    here: Cells | None = None,
) -> Known | None:
    """What this operation computes, where every input is known."""
    if _defined(op) is None:
        return None
    parts: list[Known] = []
    for one in op.args:
        got = _operand(op, one, known, here)
        if got is None:
            return None
        parts.append(got)
    if not parts:
        return None
    width = min(one.width for one in parts)

    if op.kind in (mir.Kind.COPY, mir.Kind.LOAD) and len(parts) == 1:
        return Known(masked(parts[0].n, width), width)
    if op.kind in ARITH and len(parts) == 2:
        a, b = parts
        return Known(masked(ARITH[op.kind](a.n, b.n), width), width)
    if op.kind in UNARY and len(parts) == 1:
        return Known(masked(UNARY[op.kind](parts[0].n), width), width)
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
                target = _defined(op)
                if target is None or target in facts:
                    continue
                found = _result(op, facts, None, held.get((block.at, index)))
                if found is not None:
                    facts[target] = found
                    changing = True
    return facts
