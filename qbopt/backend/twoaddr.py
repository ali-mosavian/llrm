"""Two-address fixup: x86 writes into one of the registers it reads.

`add ax,bx` is `ax := ax + bx`, not `c := a + b`. Everything above LIR has
been three-address -- MIR says what an operation computes and names its
result separately -- so before a register can be assigned, the instruction
has to say that its destination and its first source are one place.

Where they are already the same value there is nothing to do, which is the
common case: the raise built the operation from the instruction, so its
first source *is* its destination. What needs work is an operation a pass
rewrote into a genuine three-address form, and then a copy has to go in
front of it: `c := a + b` is `c := a` then `c := c + b`.

LLVM's `TwoAddressInstructionPass`, and the same order: after phi
elimination, before coalescing -- which exists partly to remove the copies
this pass and that one just introduced.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model.passes import LIRTransform

# Operations that read their destination. Everything else writes it outright.
# ir.py's vocabulary is coarser than a mnemonic: BINARY covers add, sub,
# and, or, xor and adc alike, and every one of them reads its destination.
# Widening multiply and divide use fixed registers; a single-result
# multiply with two sources instead reads its destination.
_TIED = frozenset({ir.Operation.BINARY, ir.Operation.UNARY})


class TwoAddress(LIRTransform):
    name = "twoaddr"

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return tied(body)


def tied(body: lir.LirBody) -> lir.LirBody:
    """`body` with every tied instruction reading what it writes."""
    changed = False
    from qbopt.backend import allocate
    from qbopt.backend.spiller import _next_value

    counter = [_next_value(body)]

    def mint() -> int:
        counter[0] += 1
        return counter[0] - 1

    _, leaving = allocate.live(body)
    copies = _copy_destinations(body)
    from qbopt.backend import coalesce

    interference = coalesce._interference(body)
    blocks = []
    for block in body.blocks:
        alive = set(leaving[block.at])
        live_after = {}
        for one in reversed(block.insns):
            live_after[id(one)] = frozenset(alive)
            alive.difference_update(one.defines)
            alive.update(one.uses)
        insns: list[lir.Insn] = []
        for one in block.insns:
            chosen = _commuted(one, live_after[id(one)], copies, interference)
            changed |= chosen is not one
            one = chosen
            fix = _untied(one, mint)
            if fix is None:
                insns.append(one)
                continue
            insns += fix
            changed = True
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks)) if changed else body


def _copy_destinations(body: lir.LirBody) -> dict[int, set[int]]:
    """Which values each value is copied to or from.

    Copy affinities are hints; liveness and allocation still decide legality.
    """
    adjacent: dict[int, set[int]] = {}
    for block in body.blocks:
        for one in block.insns:
            what = one.what
            if (
                what is not None
                and what.op is ir.Operation.MOVE
                and len(what.dests) == len(what.sources) == 1
                and isinstance(what.dests[0], ir.Held)
                and isinstance(what.sources[0], ir.Held)
                and what.dests[0].width == what.sources[0].width
            ):
                adjacent.setdefault(what.sources[0].value, set()).add(what.dests[0].value)
                adjacent.setdefault(what.dests[0].value, set()).add(what.sources[0].value)
    return adjacent


def _distance(copies: dict[int, set[int]], start: int, goal: int, limit: int = 8) -> float:
    """How many copies apart two values are; infinite where none joins them within `limit`."""
    seen, frontier, steps = {start}, {start}, 0
    while frontier and steps <= limit:
        if goal in frontier:
            return steps
        frontier = {other for one in frontier for other in copies.get(one, ()) if other not in seen}
        seen |= frontier
        steps += 1
    return float("inf")


def _commuted(one: lir.Insn, alive: frozenset[int], copies=None, interference=None) -> lir.Insn:
    what = one.what
    commutative = what is not None and (
        (what.op is ir.Operation.BINARY and what.name in {"add", "and", "or", "xor"})
        or (what.op is ir.Operation.MULTIPLY and what.name == "imul")
    )
    if (
        what is None
        or not commutative
        or len(what.dests) != 1
        or len(what.sources) != 2
        or one.group is not None
        or one.requires
        or one.delivers
    ):
        return one
    into, first, second = what.dests[0], *what.sources
    if (
        not all(isinstance(arg, ir.Held) for arg in (into, first, second))
        or not into.width == first.width == second.width
        or into.value == first.value
    ):
        return one
    # Tie the source nearest the destination in the copy graph. An
    # accumulator is a phi, the sum the loop writes, and the copies phi
    # elimination put between them: `acc := d + acc` has to tie `acc` for
    # the three to be one value, and nbody's accY tied `d` and copied the
    # sum back into its slot on every pass.
    affinities = (copies or {}).get(into.value, set())

    def blocked(source: int) -> int:
        return sum(other in (interference or {}).get(source, set()) for other in affinities)

    reusable = (
        first.value not in alive
        and second.value not in alive
        and (
            blocked(second.value),
            _distance(copies or {}, into.value, second.value),
        )
        < (
            blocked(first.value),
            _distance(copies or {}, into.value, first.value),
        )
    )
    if second.value == into.value or first.value in alive and second.value not in alive or reusable:
        return replace(one, what=replace(what, sources=(second, first)))
    return one


def _nothing(beside: lir.Insn) -> tuple[int, int]:
    """An empty span at the neighbour's address: this claims no bytes.

    Never None. None means "ask the node how long it was", and the node is
    the instruction this was inserted beside -- whose bytes it already
    claims, so both would, and layout reports one byte claimed twice.
    """
    at = beside.covers[0] if beside.covers else beside.at
    return (at, at)


def _untied(one: lir.Insn, mint) -> "list[lir.Insn] | None":
    """The copy and the fixed instruction, or None where it is already tied."""
    what = one.what
    if what is None or not what.dests or not what.sources:
        return None
    multiply = what.op is ir.Operation.MULTIPLY and len(what.dests) == 1 and len(what.sources) == 2
    if what.op not in _TIED and not multiply:
        return None
    into, first = what.dests[0], what.sources[0]
    if isinstance(into, ir.Mem):
        return _through_register(one, what, into, mint)
    if not isinstance(into, ir.Held) or not isinstance(first, (ir.Held, ir.Imm)):
        return None
    if isinstance(first, ir.Held) and into.value == first.value:
        return None
    move = lir.Insn(
        at=one.at,
        covers=_nothing(one),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (into,), (first,)),
        defines=(into.value,),
        uses=(first.value,) if isinstance(first, ir.Held) else (),
        op=one.op,
    )
    fixed = replace(one, what=replace(what, sources=(into, *what.sources[1:])))
    remaining = {value.value for operand in what.sources[1:] for value in ir.values(operand)}
    uses = tuple(
        value for value in one.uses if not isinstance(first, ir.Held) or value != first.value or value in remaining
    )
    return [move, replace(fixed, uses=tuple(dict.fromkeys((into.value, *uses))))]


def _through_register(one: lir.Insn, what: ir.Semantics, into: ir.Mem, mint) -> "list[lir.Insn] | None":
    """A memory destination computed in a register, then stored.

    `add [x],bx` reads [x]. Where a pass served that read from a value
    instead -- forwarding `y := [x]` into the cell's own accumulation --
    the instruction still read memory and sphere-mapped plasma accumulated
    onto a stale [bp-0EEh].
    """
    if len(what.dests) != 1 or what.sources[0] == into or one.group is not None:
        return None
    if any(isinstance(source, ir.Mem) for source in what.sources) or one.requires or one.delivers:
        return None
    held = ir.Held(mint(), into.width)
    first = what.sources[0]
    load = lir.Insn(
        at=one.at,
        covers=_nothing(one),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (held,), (first,)),
        defines=(held.value,),
        uses=tuple(value.value for value in ir.values(first)),
        op=one.op,
    )
    computed = replace(
        one,
        what=replace(what, dests=(held,), sources=(held, *what.sources[1:])),
        defines=(held.value,),
        uses=tuple(
            dict.fromkeys((held.value, *(value.value for source in what.sources[1:] for value in ir.values(source))))
        ),
    )
    store = lir.Insn(
        at=one.at,
        covers=_nothing(one),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (into,), (held,)),
        defines=(),
        uses=tuple(dict.fromkeys((held.value, *(value.value for value in ir.values(into))))),
        op=one.op,
    )
    return [load, computed, store]
