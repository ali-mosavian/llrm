"""Which cells nothing outside a body can read.

A store is dead when everything after it overwrites the cell before reading
it -- or when nothing after it can read the cell at all. `avail.dead_stores`
answers the first; this answers what the second needs: at a call or an exit,
which cells are still observable.

A frame cell below bp is gone when the body returns, and nothing a call runs
can reach it unless its address was taken. A program variable is visible to
every other body that names it, to anything handed its address, and to a
later activation of the same body -- so only the main body, which nothing
calls, can own one, and only a variable no other code and no data names.

Error and event handlers resume inside a body without a CFG edge, so a
module with either has nothing private.

The one assumption is the one `module.landmarks` states: a taken address
reaches its own variable and not the next one. A number used as an address
into DGROUP or the stack names no variable a program can know the place of.
"""

from bisect import bisect_right
from dataclasses import dataclass
from collections.abc import Callable

from iced_x86 import Register

from qbopt.model import mir
from qbopt.model import memory
from qbopt.analysis import alias
from qbopt.objectfile import omf
from qbopt.analysis import frameescape
from qbopt.objectfile.module import Space
from qbopt.objectfile.module import Module

# Past every displacement a 16-bit segment can hold.
BEYOND = 1 << 17


@dataclass(frozen=True, slots=True)
class Exposure:
    data: int | None
    main: int | None
    # Program-data ranges some code outside the owning body, some data, or a
    # taken address can reach.
    everywhere: tuple[tuple[int, int], ...]
    # The ranges each body's own direct operands name, by body seed.
    direct: dict[int, tuple[tuple[int, int], ...]]


_known: dict[int, tuple[Module, Exposure | None]] = {}


def exposure(found: Module, blocks: list) -> Exposure | None:
    """The module's program-data visibility, or None where it cannot be told."""
    held = _known.get(id(found))
    if held is not None and held[0] is found:
        return held[1]
    got = _exposure(found, blocks)
    _known[id(found)] = (found, got)
    return got


def _exposure(found: Module, blocks: list) -> Exposure | None:
    from qbopt.abi import events
    from qbopt.abi import handlers
    from qbopt.frontend import extent

    if events.handler_entries(found) or handlers.error_entries(found):
        return None
    partition = extent.partition(found)
    if isinstance(partition, str) or not partition.complete:
        return None
    data = found.program_data
    if data is not None and omf.combines(found.records).get(data) == omf.COMBINE_COMMON:
        data = None

    # CodeView's $$SYMBOLS names every variable, and is not loaded.
    segments = omf.segments(found.records)
    debug = frozenset(index for index, one in enumerate(segments) if one is not None and one[0].startswith("$$"))
    fixups = [one for one in omf.fixups(found.records) if one.seg not in debug]
    publics = omf.public_definitions(found.records).values()
    named = sorted(
        {one.disp for one in fixups if one.target == "segment" and one.index == data}
        | {offset for seg, offset in publics if seg == data}
    )

    def reach(disp: int) -> tuple[int, int]:
        # To the next named displacement: an operand cannot tell how wide
        # the variable it names is, and the one after it bounds it.
        after = bisect_right(named, disp)
        return disp, named[after] if after < len(named) else BEYOND

    insns = {insn.at: insn for block in blocks for insn in block.insns}
    by_field = {field: insn for insn in insns.values() for field in (insn.disp_at, insn.imm_at) if field is not None}
    everywhere = [reach(offset) for seg, offset in publics if seg == data]
    direct: dict[int, list[tuple[int, int]]] = {}
    for one in fixups:
        if one.target != "segment" or one.index != data:
            continue
        insn = by_field.get(one.offset) if one.seg == found.seg else None
        operand = insn is not None and one.offset == insn.disp_at
        owner = (
            next((body.seed for body in partition.bodies if any(lo <= insn.at < hi for lo, hi in body.ranges)), None)
            if operand and insn.memory_base == Register.NONE and insn.memory_index == Register.NONE
            else None
        )
        if owner is not None:
            direct.setdefault(owner, []).append(reach(one.disp))
        elif insn is not None and not operand:
            # An address the code hands over. How far its holder writes from it is
            # not in the object, and the next named displacement is no bound: BC
            # names a long's high word by its own operand, and PROCS lost that store.
            everywhere.append((one.disp, BEYOND))
        else:
            everywhere.append(reach(one.disp))

    main = next(
        (
            body.seed
            for body in partition.bodies
            if body.kind == "main" and body.seed not in found.publics and body.seed not in found.targets
        ),
        None,
    )
    return Exposure(data, main, tuple(everywhere), {seed: tuple(ranges) for seed, ranges in direct.items()})


def private(body: mir.MirBody, found: Module | None, blocks: list | None) -> Callable[[mir.MemRef], bool] | None:
    """A test for the cells no call and no exit of `body` can observe.

    Without a module there is no program data to know the reach of, but a
    body the raise calls sealed still owns its frame: nothing resumes inside
    it, so the frame rule needs nothing the module would have said.
    """
    if found is None or blocks is None:
        if not body.sealed:
            return None
        exposed = Exposure(None, None, (), {})
    else:
        exposed = exposure(found, blocks)
    if exposed is None:
        return None
    escapes = frameescape.analysed(body)
    pointers = alias.points_to(body)
    published = set(pointers.escaped)
    unknown_frame_publication = False
    publishing = frozenset({mir.Kind.ARG, mir.Kind.CALL, mir.Kind.RETURN, mir.Kind.ESCAPE, mir.Kind.OPAQUE})
    for block in body.blocks:
        for op in block.ops:
            if op.kind not in publishing and not op.barrier and not op.exits:
                continue
            values = set(op.uses) | set(op.exits)
            values.update(arg.value for arg in op.args if isinstance(arg, mir.Held))
            for value in values:
                provenance = pointers.values.get(value)
                if provenance is not None:
                    published.update(one.object for one in provenance.slices)
            # A direct address operand has no SSA value through which to
            # match a frontend-defined object identity.  Conservatively
            # expose every canonical frame object rather than guessing from
            # coincident offsets.
            unknown_frame_publication |= any(isinstance(arg, mir.FrameAddress) for arg in op.args)
    frame = escapes.reach is not None and not escapes.opaque_addresses
    reach = escapes.reach or frozenset()
    statics = exposed.data is not None and body.entry == exposed.main
    seen = exposed.everywhere + tuple(
        one for seed, ranges in exposed.direct.items() if seed != body.entry for one in ranges
    )

    def unobserved(ref: mir.MemRef) -> bool:
        provenance = pointers.reference(ref)
        if provenance is not None and provenance.slices and not unknown_frame_publication:
            canonical_frame = all(
                one.object.kind is memory.Kind.FRAME
                and one.object.extent is not None
                and 0 <= one.low < one.high
                and one.high + one.width - 1 <= one.object.extent
                for one in provenance.slices
            )
            if canonical_frame:
                # Canonical object identity is a complete answer in both
                # directions. Falling through to the raw BP-offset rule after
                # finding a published object made that same object private
                # again and let DSE erase stores before a BYREF call.
                return all(one.object not in published for one in provenance.slices)
        addr = ref.addr
        if addr is None or addr.base != Register.NONE or ref.base is not None or ref.segment is not None:
            return False
        lo, hi = addr.disp, addr.disp + ref.width
        if addr.space is Space.FRAME:
            return frame and hi <= 0 and not any(start < hi and lo < end for start, end in reach)
        if addr.space is Space.SEGMENT and addr.index == exposed.data:
            return statics and not any(start < hi and lo < end for start, end in seen)
        return False

    return unobserved
