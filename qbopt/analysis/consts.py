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

Memory facts come from explicit stores and loader-established entry facts
supplied by the raise, not assumed zero-filled or immutable static contents.
Adjacent known fragments can supply a wider read only when every requested byte
is covered. Unknown and overlapping writes still invalidate the cell facts.
Phi inputs and memory facts meet on agreement across incoming paths.
"""

from dataclasses import dataclass

from qbopt.model import ir
from qbopt.model import mir
from qbopt.abi import runtime

# What each operation does to two known numbers, within one width. Division
# has two results and is handled separately, with the faulting cases excluded.
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


# Memory facts are stored as bytes so partial writes and control-flow
# joins do not discard an untouched neighbor. Reads assemble their width.
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


def division(op: mir.Op, known: dict, here: Cells) -> tuple[int, int] | None:
    """Signed quotient and remainder, excluding the two faulting cases."""
    if op.kind is not mir.Kind.DIVMOD or len(op.args) != 2 or len(op.results) != 2 or op.stores:
        return None
    if any(not isinstance(result, mir.Held) or result.width != 4 for result in op.results):
        return None
    operands = [_operand(op, arg, known, here) for arg in op.args]
    if any(fact is None or fact.width < 4 for fact in operands):
        return None
    dividend, divisor = [((fact.n & 0xFFFFFFFF) ^ 0x80000000) - 0x80000000 for fact in operands]
    if divisor == 0 or (dividend == -0x80000000 and divisor == -1):
        return None
    quotient = abs(dividend) // abs(divisor)
    if (dividend < 0) != (divisor < 0):
        quotient = -quotient
    return masked(quotient, 4), masked(dividend - quotient * divisor, 4)


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


def initialized(op: mir.Op, ref: mir.MemRef) -> Known | None:
    """The complete value a direct constant store writes to a contained cell."""
    if op.kind is not mir.Kind.STORE or op.loads or op.barrier or len(op.stores) != 1:
        return None
    written = mir._symbolic_ref(op.stores[0])
    if written.addr is None or written.base is not None or written.segment is not None:
        return None
    fact = _put(op, {})
    return _cell({(written.addr, written.width): fact}, ref) if fact is not None else None


def _fragments(ref: mir.MemRef, fact: Known) -> Cells:
    return {(ref.addr.plus(offset), 1): Known((fact.n >> (offset * 8)) & 255, 1)
            for offset in range(min(ref.width, fact.width))}


def _kills(
    here: Cells, op: mir.Op, known: dict[mir.Value, Known], dgroup: frozenset[int], calls: dict[int, str]
) -> Cells:
    """The cell facts still standing after this operation."""
    if op.at in calls:
        contract = runtime.contract(calls[op.at])
        if runtime.writes_caller_memory(contract) or runtime.barrier(contract):
            here = {}
    for ref in op.stores:
        ref = mir._symbolic_ref(ref)
        if ref.addr is None:
            here = {}
            break
        here = {
            where: fact
            for where, fact in here.items()
            if not mir.overlapping(mir.MemRef(where[0], where[1], None, None), ref, dgroup)
        }
        put = _put(op, known)
        if put is not None and ref.base is None and ref.segment is None:
            here.update(_fragments(ref, put))
    if op.kind is mir.Kind.CALL and op.memory_values:
        here = dict(here)
        for ref, value in op.memory_values:
            if ref.addr is not None and ref.base is None and ref.segment is None:
                here.update(_fragments(ref, Known(masked(value.n, value.width), value.width)))
    return here


def cells(
    body: mir.MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    known: dict[mir.Value, Known] | None = None,
    *, initial: Cells | None = None, edges: dict[tuple[int, int], Cells] | None = None,
) -> dict[tuple[int, int], Cells]:
    """What each memory cell holds before each operation, where it is a number.

    Forward to a fixed point, meeting at a join on agreement, which is the
    same shape as known() and for the same reason. Keyed on the block and
    the operation's index within it rather than its address, because
    absorption puts several operations on one address.

    Optional edge facts are independently proved byte fragments. Apply them
    before the predecessor meet, so a loop's final value is not its invariant
    value and a bypass path must agree before a following read can fold.

    A block none of whose predecessors have been visited yet is *deferred*,
    not treated as knowing nothing. Saying "nothing is known here" poisons
    the meet for good -- a loop body's only predecessor is its header, which
    is unvisited on the first round, so hotlop could never learn that `n` is
    7 even though the entry block says so three instructions earlier.
    """
    known = known if known is not None else {}
    if initial is None:
        initial = {}
        for ref, value in body.initial:
            initial.update(_fragments(ref, Known(value.n, value.width)))
    outof: dict[int, Cells | None] = {block.at: None for block in body.blocks}
    preds = {block.at: [one.at for one in body.blocks if block.at in one.succ] for block in body.blocks}

    def entering(at: int) -> Cells | None:
        if not preds[at]:
            return dict(initial or {}) if at == body.entry else {}
        seen = []
        for one in preds[at]:
            if outof[one] is None:
                continue
            extra = (edges or {}).get((one, at), {})
            here = outof[one]
            if extra:
                here = {where: fact for where, fact in here.items()
                        if not any((where[0].plus(offset), 1) in extra for offset in range(where[1]))}
                here = {**here, **extra}
            seen.append(here)
        if at == body.entry:
            seen.append(initial or {})
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


def _cell(here: Cells, ref: mir.MemRef) -> Known | None:
    ref = mir._symbolic_ref(ref)
    if ref.addr is None or ref.base is not None or ref.segment is not None:
        return None
    if exact := _read(here.get((ref.addr, ref.width)), ref.width):
        return exact
    number = 0
    for offset in range(ref.width):
        wanted = ref.addr.plus(offset)
        fragments = {
            (fact.n >> (8 * byte)) & 255
            for (address, width), fact in here.items()
            for byte in range(min(width, fact.width))
            if address.plus(byte) == wanted
        }
        if len(fragments) != 1:
            return None
        number |= fragments.pop() << (8 * offset)
    return Known(number, ref.width)


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
        return _cell(here, one.ref)
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
    carries: dict[mir.Value, int] | None = None,
) -> Known | None:
    """What this operation computes, where every input is known."""
    if _defined(op) is None:
        return None
    if op.kind is mir.Kind.EXTRACT and len(op.args) == 2 and len(op.results) == 1:
        source, offset = op.args
        fact = _operand(op, source, known, here)
        width = op.results[0].width
        if (
            isinstance(offset, mir.Const)
            and offset.n >= 0
            and fact is not None
            and fact.width * 8 >= offset.n + width * 8
        ):
            return Known(masked(fact.n >> offset.n, width), width)
        return None
    if (
        op.kind in (mir.Kind.XOR, mir.Kind.SUB)
        and len(op.args) == 2
        and isinstance(op.args[0], mir.Held)
        and op.args[0] == op.args[1]
    ):
        return Known(0, op.args[0].width)
    parts: list[Known] = []
    for one in op.args:
        got = _operand(op, one, known, here)
        if got is None:
            return None
        parts.append(got)
    if not parts:
        return None
    if op.kind is mir.Kind.SIGN_EXTEND and len(parts) == len(op.results) == 1:
        source, result = op.args[0], op.results[0]
        if (not isinstance(source, (mir.Held, mir.Const)) or not isinstance(result, mir.Held)
            or not 0 < source.width < result.width <= 4 or parts[0].width < source.width):
            return None
        sign = 1 << (source.width * 8 - 1)
        signed = (masked(parts[0].n, source.width) ^ sign) - sign
        return Known(masked(signed, result.width), result.width)
    if op.kind is mir.Kind.CONCAT and len(parts) == 2 and len(op.results) == 1:
        high, low = op.args
        width = high.width + low.width
        if op.results[0].width != width or any(fact.width < arg.width for fact, arg in zip(parts, op.args)):
            return None
        return Known((masked(parts[0].n, high.width) << (low.width * 8)) | masked(parts[1].n, low.width), width)
    width = min(one.width for one in parts)
    if op.kind is mir.Kind.ADD_CARRY and len(parts) == 2:
        flags = [value for value in op.uses if value.flags]
        if len(flags) == 1 and flags[0] in (carries or {}):
            return Known(masked(parts[0].n + parts[1].n + carries[flags[0]], width), width)
        return None

    if len(parts) == 1 and (step := mir.stepping(op)) is not None and isinstance(step[1], mir.Const):
        return Known(masked(parts[0].n + step[1].n, width), width)

    if op.kind in (mir.Kind.COPY, mir.Kind.LOAD) and len(parts) == 1:
        return Known(masked(parts[0].n, width), width)
    if op.kind in ARITH and len(parts) == 2:
        a, b = parts
        return Known(masked(ARITH[op.kind](a.n, b.n), width), width)
    if op.kind in UNARY and len(parts) == 1:
        return Known(masked(UNARY[op.kind](parts[0].n), width), width)
    return None


def _carry(op: mir.Op, facts: dict, here: Cells) -> int | None:
    if op.kind is not mir.Kind.ADD or len(op.args) != 2 or len(op.results) != 1:
        return None
    if not isinstance(op.results[0], mir.Held):
        return None
    width = op.results[0].width
    operands = [_operand(op, arg, facts, here) for arg in op.args]
    if any(fact is None or fact.width < width for fact in operands):
        return None
    return int(sum(masked(fact.n, width) for fact in operands) >= 1 << (width * 8))


def known(
    body: mir.MirBody,
    dgroup: frozenset[int] | None = None,
    calls: dict[int, str] | None = None,
    *, edges: dict[tuple[int, int], Cells] | None = None, initial: Cells | None = None,
) -> dict[mir.Value, Known]:
    """Every value this body computes that is a number, to a fixed point.

    Forward over the blocks until nothing new is learned. A value's fact
    only ever goes from unknown to known and never changes once set --
    which is SSA's own doing, since the value is defined once -- so the
    walk terminates on the count of values rather than on any ordering.
    """
    facts: dict[mir.Value, Known] = {}
    carries: dict[mir.Value, int] = {}
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
            held = cells(body, dgroup, calls, facts, edges=edges, initial=initial)
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
                here = held.get((block.at, index), {})
                carry = _carry(op, facts, here)
                if carry is not None:
                    for value in op.defines:
                        if value.flags and value not in carries:
                            carries[value] = carry
                            changing = True
                target = _defined(op)
                if target is None or target in facts:
                    continue
                found = _result(op, facts, None, here, carries)
                if found is not None:
                    facts[target] = found
                    changing = True
    return facts
