"""
Which value holds a cell's contents, across blocks.

memory.py answers "what does this address hold" cross-block, and answers it
about addresses. regalloc.py answers "where does this value live", and
answers it about values. Neither alone decides whether a load can go: the
cell's content being known says the load is redundant, and the value still
being live says a register still has it. Only both together make a load
removable, and forward.py makes that join one basic block at a time.

This is the join, cross-block. A forward dataflow whose fact is

    MemRef -> Value

meaning "these bytes are this SSA value". Keyed on MemRef rather than on
Addr because MemRef carries the SSA values its own address is reached
through, so the fact survives anything that reallocates registers -- which
is the whole reason to state it over MIR instead of over machine code.

Two things it deliberately does not do.

**It does not claim the value is available.** An entry says which value the
bytes are, not that a register still holds it. BC spills across calls, and
the reload it writes afterwards is real work:

    mov [bp-18h],ax     the spill
    call B$MUI4         ax is gone
    add ax,[bp-18h]     the reload -- necessary

memory.py calls that reload redundant and is right about the cell. The
value is dead there, so the load stays. Ask regalloc.live() before
believing an entry means anything.

**It intersects at joins rather than placing a phi.** Where predecessors
disagree about which value a cell holds, the fact is dropped. A phi would
be the stronger answer, and mir.py already has the machinery -- but a phi
for a memory cell is a new value with no defining instruction, which
lowering has no way to emit. Conservative here is the honest floor.
"""

from dataclasses import dataclass

from qbopt import mir
from qbopt.mir import Op
from qbopt import runtime
from qbopt.mir import Value
from qbopt.mir import MemRef
from qbopt.mir import MirBody

# What a cell maps to, and the whole lattice element.
Holders = dict[MemRef, Value]


@dataclass(frozen=True, slots=True)
class Held:
    """The map on entry to and exit from each block."""

    into: dict[int, Holders]
    outof: dict[int, Holders]


def _addressing(op: Op) -> set[Value]:
    """Values used only to reach an operand, not read as data."""
    found: set[Value] = set()
    for ref in op.loads + op.stores:
        if ref.base is not None:
            found.add(ref.base)
        if ref.segment is not None:
            found.add(ref.segment)
    return found


def _real(values: tuple[Value, ...]) -> list[Value]:
    return [one for one in values if one.of is not mir.FLAGS]


def loaded_into(op: Op) -> tuple[MemRef, Value] | None:
    """The cell this op purely loads, and the value it lands in.

    Purely: one read, no write, one value defined, nothing read as data, and
    an address something can name -- a MemRef whose addr is None aliases
    everything, so it can never be matched by same_bytes and would sit in
    the map as an entry no lookup can use and every store has to step over.
    `and cx,[x]` fails the last test -- it uses cx as data as well as
    defining it, so the bytes it leaves in cx are not the cell's. Treating
    it as a load is the bug tools/matrix.py caught in forward.py, and the
    same shape has to be refused here.
    """
    if len(op.loads) != 1 or op.stores or op.barrier or op.loads[0].addr is None:
        return None
    defines = _real(op.defines)
    if len(defines) != 1:
        return None
    if set(_real(op.uses)) - _addressing(op):
        return None
    return op.loads[0], defines[0]


def stored_from(op: Op) -> tuple[MemRef, Value] | None:
    """The cell this op purely stores, and the value it wrote there."""
    if len(op.stores) != 1 or op.loads or op.barrier or op.stores[0].addr is None:
        return None
    if _real(op.defines):
        return None
    reading = [one for one in _real(op.uses) if one not in _addressing(op)]
    if len(reading) != 1:
        return None
    return op.stores[0], reading[0]


def _clean(op: Op, calls: dict[int, str]) -> bool:
    """Whether this call provably leaves caller memory alone.

    mir.py gives a call a store of `MemRef(addr=None)`, which aliases
    everything and wipes this map. That is the right default and the wrong
    answer for the routines runtime.py has actually read: B$MUI4 multiplies
    two longs in registers and touches no caller memory at all, so a cell
    established before it is still that value after.

    Registers are a separate question and are not answered here. A call
    clobbers ax, cx, dx and bx whatever it does to memory, so the value an
    entry names is usually dead afterwards -- the entry survives, and
    regalloc.live() is what says whether it means anything.
    """
    name = calls.get(op.at)
    if name is None:
        return False
    routine = runtime.contract(name)
    return routine.established and not runtime.barrier(routine) and not runtime.writes_caller_memory(routine)


def _after(op: Op, holders: Holders, dgroup: frozenset[int], calls: dict[int, str]) -> Holders:
    """The map across one op."""
    if op.barrier:
        return {}
    if op.at in calls:
        return holders if _clean(op, calls) else {}

    for ref in op.stores:
        holders = {one: who for one, who in holders.items() if not mir.overlapping(one, ref, dgroup)}
    found = stored_from(op) or loaded_into(op)
    if found is not None:
        ref, value = found
        holders = dict(holders)
        holders[ref] = value
    return holders


def _meet(maps: list[Holders]) -> Holders:
    """Only what every predecessor agrees on, value and all."""
    if not maps:
        return {}
    out = dict(maps[0])
    for other in maps[1:]:
        out = {one: who for one, who in out.items() if other.get(one) == who}
    return out


def holders(body: MirBody, dgroup: frozenset[int], calls: dict[int, str] | None = None) -> Held:
    """Which value each cell holds, at every block's entry and exit."""
    calls = calls or {}
    preds: dict[int, list[int]] = {block.at: [] for block in body.blocks}
    for block in body.blocks:
        for succ in block.succ:
            if succ in preds:
                preds[succ].append(block.at)

    into: dict[int, Holders] = {block.at: {} for block in body.blocks}
    outof: dict[int, Holders] = {block.at: {} for block in body.blocks}

    changing = True
    while changing:
        changing = False
        for block in body.blocks:
            arriving = {} if block.at == body.entry else _meet([outof[one] for one in preds[block.at]])
            leaving = dict(arriving)
            for op in block.ops:
                leaving = _after(op, leaving, dgroup, calls)
            if arriving != into[block.at] or leaving != outof[block.at]:
                into[block.at], outof[block.at] = arriving, leaving
                changing = True

    return Held(into, outof)


def provider(
    body: MirBody, dgroup: frozenset[int], at: int, ref: MemRef, calls: dict[int, str] | None = None
) -> Value | None:
    """The value holding `ref`'s bytes just before the op at `at`.

    Says nothing about whether that value is still live there -- see the
    module docstring, and ask regalloc.live().
    """
    calls = calls or {}
    found = holders(body, dgroup, calls)
    for block in body.blocks:
        if not any(op.at == at for op in block.ops):
            continue
        current = dict(found.into[block.at])
        for op in block.ops:
            if op.at == at:
                return next((who for one, who in current.items() if mir.same_bytes(one, ref)), None)
            current = _after(op, current, dgroup, calls)
    return None
