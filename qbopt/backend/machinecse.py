"""Eliminate repeated deterministic computations after register allocation.

MIR GVN sees values, not the physical instructions lowering and allocation
create.  A frame address can therefore be selected twice into the same hard
register even though neither its inputs nor that register changed in between.
At this point the question is entirely physical: value-number the byte lanes
read and written by independently reproducible instructions, and retain an
anchor for the virtual/data-source ownership of a redundant occurrence.
"""

from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import target
from qbopt.backend.peephole import _lanes
from qbopt.objectfile.module import Space
from qbopt.model.passes import LIRTransform

_REPRODUCIBLE = frozenset(
    {
        (ir.Operation.MOVE, "mov"),
        (ir.Operation.EXTEND, "movsx"),
        (ir.Operation.EXTEND, "movzx"),
        (ir.Operation.ADDRESS, "lea"),
    }
)


class MachineCSE(LIRTransform):
    """Remove an exact physical recomputation whose inputs still agree."""

    name = "machine-cse"

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return eliminated(body)


def _relocated(where: ir.Loc) -> bool:
    if isinstance(where, ir.Imm):
        return where.address is not None
    if isinstance(where, ir.Address):
        return where.addr is not None and where.addr.space is not Space.FRAME
    return False


def _shape(where: ir.Loc) -> tuple:
    """Every encoded field of a source, including compare=False address fields."""
    if isinstance(where, ir.Reg):
        return ("reg", where.register, where.width)
    if isinstance(where, ir.Imm):
        return ("imm", where.value, where.width, where.address)
    if isinstance(where, ir.Address):
        return (
            "address",
            where.addr,
            where.through,
            where.index,
            where.scale,
            where.offset,
            where.disp_width,
        )
    raise TypeError(f"machine CSE source is not independently reproducible: {where!r}")


def _source_lanes(where: ir.Loc) -> "set[tuple] | None":
    """Physical input lanes, or None for a register this tracker omits."""
    registers = ()
    if isinstance(where, ir.Reg):
        registers = (where.register,)
    elif isinstance(where, ir.Address):
        registers = tuple(register for register in (where.through, where.index) if register is not Register.NONE)
    elif not isinstance(where, ir.Imm):
        return None
    lanes = set()
    for register in registers:
        found = _lanes(register)
        if not found:
            return None
        lanes |= found
    return lanes


def _candidate(one: lir.Insn) -> "tuple[tuple, tuple, tuple] | None":
    """The pure register result this occurrence can independently reproduce.

    These are the x86 forms whose result is independent of their destination's
    previous contents and which do not set flags.  Memory reads are excluded:
    proving a cell unchanged is MIR's MemorySSA job, while this pass exists for
    artifacts created below that boundary.  Relocations are excluded because
    every emitted symbolic occurrence owns a fixup as well as instruction
    bytes.
    """
    what = one.what
    if (
        what is None
        or (what.op, what.name) not in _REPRODUCIBLE
        or len(what.dests) != 1
        or not isinstance(what.dests[0], ir.Reg)
        or any(not isinstance(arg, (ir.Reg, ir.Imm, ir.Address)) for arg in what.sources)
        or any(_relocated(arg) for arg in (*what.dests, *what.sources))
        or what.target is not None
        or what.indirect
        or one.clobbers
        or one.clobbers_high
        or one.requires
        or one.delivers
        or one.spread
        or one.group is not None
        or one.symbol is True
        or one.frame_adjust
        or one.spill_reload
        or one.spill_store
        or getattr(one.op, "barrier", False)
    ):
        return None
    read_sets = tuple(_source_lanes(source) for source in what.sources)
    if any(found is None for found in read_sets):
        return None
    reads = set().union(*(found for found in read_sets if found is not None))
    destination = what.dests[0]
    writes = _lanes(destination.register)
    if not writes or reads & writes:
        return None
    # Destination placement is deliberately absent.  The computed value is
    # identified by the operation, its explicit inputs and the values currently
    # occupying their physical lanes; the output tokens below say where it is.
    expression = (what.op, what.name, tuple(map(_shape, what.sources)), destination.width)
    return expression, tuple(sorted(reads)), tuple(sorted(writes))


def _written(one: lir.Insn) -> "set[tuple] | None":
    """Explicit and declared physical writes, or None for an opaque boundary."""
    what = one.what
    if what is None or what.op in {
        ir.Operation.BARRIER,
        ir.Operation.CALL,
        ir.Operation.RETURN,
        ir.Operation.FILL,
        ir.Operation.LEAVE,
    }:
        return None
    writes = set()
    for dest in what.dests:
        if isinstance(dest, ir.Reg):
            writes |= _lanes(dest.register)
    for held, register in one.delivers:
        writes |= _lanes(target.named(register, held.width))
    for register in one.clobbers:
        writes |= _lanes(register)
    for register in one.clobbers_high:
        writes |= {lane for lane in _lanes(register) if lane[1] >= 2}
    return writes


def _transfer(block: lir.LirBlock, incoming: dict) -> tuple[dict, frozenset[int]]:
    """Physical values leaving one block and redundant occurrences within it."""
    state = dict(incoming)
    redundant: set[int] = set()
    for one in block.insns:
        candidate = _candidate(one)
        if candidate is not None:
            expression, reads, writes = candidate
            inputs = tuple((lane, state.get(lane, ("block-entry", block.at, lane))) for lane in reads)
            value = ("expression", expression, inputs)
            wanted = {lane: (value, byte) for byte, lane in enumerate(writes)}
            if all(state.get(lane) == token for lane, token in wanted.items()):
                redundant.add(id(one))
                continue
            state.update(wanted)
            continue
        writes = _written(one)
        if writes is None:
            state.clear()
            continue
        for lane in writes:
            state[lane] = ("written", id(one), lane)
    return state, frozenset(redundant)


def _merged(states: list[dict]) -> dict:
    """The physical lane values every incoming edge agrees on."""
    if not states:
        return {}
    common = dict(states[0])
    for state in states[1:]:
        common = {lane: token for lane, token in common.items() if state.get(lane) == token}
    return common


def _lanes_used(body: lir.LirBody) -> frozenset[tuple]:
    lanes = set()
    for one in body.insns:
        candidate = _candidate(one)
        if candidate is not None:
            _expression, reads, writes = candidate
            lanes.update(reads)
            lanes.update(writes)
        elif (writes := _written(one)) is not None:
            lanes.update(writes)
    return frozenset(lanes)


def eliminated(body: lir.LirBody) -> lir.LirBody:
    """Value-number deterministic register computations across the CFG."""
    predecessors = {block.at: set() for block in body.blocks}
    for block in body.blocks:
        for successor in block.succ:
            if successor in predecessors:
                predecessors[successor].add(block.at)
    entry = {lane: ("entry", lane) for lane in _lanes_used(body)}
    outgoing: dict[int, dict] = {}
    redundant: dict[int, frozenset[int]] = {}
    # Acyclic facts normally settle in layout order in one pass. Reversed
    # blocks and conservative loop joins may need more; refusal to converge
    # keeps the body unchanged rather than trusting a partial physical state.
    for _round in range(max(1, len(body.blocks) * 4)):
        changed = False
        for block in body.blocks:
            states = [outgoing[at] for at in predecessors[block.at] if at in outgoing]
            if len(states) != len(predecessors[block.at]):
                states.append({})
            if block.at == body.entry:
                states.append(entry)
            incoming = _merged(states)
            after, gone = _transfer(block, incoming)
            if outgoing.get(block.at) != after or redundant.get(block.at) != gone:
                outgoing[block.at], redundant[block.at], changed = after, gone, True
        if not changed:
            break
    else:
        return body
    blocks = tuple(
        replace(
            block,
            insns=tuple(lir.anchor(one) if id(one) in redundant.get(block.at, ()) else one for one in block.insns),
        )
        if redundant.get(block.at)
        else block
        for block in body.blocks
    )
    unchanged = all(before is after for before, after in zip(body.blocks, blocks, strict=True))
    return body if unchanged else replace(body, blocks=blocks)
