"""Expose explicit BC literal initializer bytes at module entry.

BC_CN also contains descriptors and relocatable data. Only direct scalar
reads backed by complete, nonoverlapping, unrelocated bytes are admitted.
These are initial memory values, not immutable loads; subsequent writes and
calls still kill them. Procedure entries do not inherit loader state.
"""

from dataclasses import replace

from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.objectfile import omf
from qbopt.objectfile import module
from qbopt.frontend.blocks import ENTRY
from qbopt.objectfile.module import Space


def initialized(
    body: mir.MirBody,
    found: module.Module,
    contracts: dict[int, runtime.Contract] | None = None,
) -> mir.MirBody:
    if body.entry != ENTRY:
        return body
    pools = {
        index
        for index, segment in enumerate(omf.segments(found.records))
        if segment is not None and segment[0] == "BC_CN" and not omf.pubdef_names(found.records, index)
    }
    if not pools:
        return body
    far_strings = {
        index
        for index, segment in enumerate(omf.segments(found.records))
        if segment is not None and segment[0] == "FSL_CONST" and not omf.pubdef_names(found.records, index)
    }
    readable = pools | far_strings
    fixups = omf.fixups(found.records)
    data, ambiguous = {}, set()
    unknown = set()
    for fixup in fixups:
        if fixup.seg not in readable:
            continue
        width = {omf.LOC_OFF16: 2, omf.LOC_BASE: 2, omf.LOC_PTR32: 4}.get(fixup.loc)
        if width is None:
            unknown.add(fixup.seg)
            continue
        ambiguous.update((fixup.seg, fixup.offset + byte) for byte in range(width))
    for _, index, start, payload in omf.ledata(found.records):
        if index not in readable or index in unknown:
            continue
        for offset, byte in enumerate(payload, start):
            key = index, offset
            if key in data:
                ambiguous.add(key)
            data[key] = byte
    values = {}
    for block in body.blocks:
        for op in block.ops:
            for ref in op.loads:
                ref = mir._symbolic_ref(ref)
                if (
                    ref.addr is None
                    or ref.addr.space is not Space.SEGMENT
                    or ref.addr.index not in pools
                    or ref.base is not None
                    or ref.segment is not None
                    or ref.width not in (1, 2, 4, 8)
                ):
                    continue
                keys = [(ref.addr.index, offset) for offset in range(ref.addr.disp, ref.addr.disp + ref.width)]
                if all(key in data and key not in ambiguous for key in keys):
                    values[ref] = mir.Const(int.from_bytes(bytes(data[key] for key in keys), "little"), ref.width)
    if not values:
        return body
    protected = _numeric_ranges(values, data, ambiguous, fixups, module.escaped(found), far_strings)
    selected = runtime.for_module(found) if contracts is None else contracts
    handles_errors = any(contract.error_handling for contract in selected.values())

    def annotate(op: mir.Op) -> mir.Op:
        if op.kind is not mir.Kind.CALL or not protected:
            return op
        contract = selected.get(op.at)
        if (
            contract is None
            or runtime.barrier(contract)
            or contract.writes is not runtime.Memory.OWN
            or (contract.raises_error and handles_errors)
        ):
            return op
        return replace(
            op,
            stores=tuple(
                replace(ref, excludes=tuple(dict.fromkeys((*ref.excludes, *protected)))) if ref.addr is None else ref
                for ref in op.stores
            ),
        )

    return replace(
        body,
        initial=tuple(values.items()),
        blocks=tuple(replace(block, ops=tuple(map(annotate, block.ops))) for block in body.blocks),
    )


def _numeric_ranges(values, data, ambiguous, fixups, escaped, far_strings=frozenset()):
    """Separate scalar literals from validated escaped string descriptors and their payloads.

    OWN runtime effects may compact strings, not write arbitrary numeric
    objects. Unknown escaped layouts prevent any exclusion for their pool.
    Explicit program writes and unknown calls still invalidate entry facts.
    """
    protected = []
    for ref in values:
        index = ref.addr.index
        occupied = set()
        for segment, offset in escaped:
            if segment != index:
                continue
            far = _far_descriptor(index, offset, data, ambiguous, fixups, far_strings)
            if far is not None:
                occupied.update(far)
                continue
            pointer = _relocation(index, offset + 2, omf.LOC_OFF16, data, fixups)
            if (
                pointer is None
                or pointer.index != index
                or (index, offset) not in data
                or (index, offset + 1) not in data
                or (index, offset) in ambiguous
                or (index, offset + 1) in ambiguous
            ):
                break
            length = data[index, offset] | data[index, offset + 1] << 8
            target = pointer.disp
            # The descriptor pointer itself is relocated; any other relocation
            # or missing payload byte makes this an unknown object layout.
            payload = range(target, target + length)
            if any((index, byte) not in data or (index, byte) in ambiguous for byte in payload):
                break
            occupied.update(range(offset, offset + 4))
            occupied.update(payload)
        else:
            if not any(byte in occupied for byte in range(ref.addr.disp, ref.addr.disp + ref.width)):
                protected.append((ref.addr, ref.width))
    return tuple(protected)


def _far_descriptor(index, offset, data, ambiguous, fixups, far_strings):
    """VBDOS's two-word indirect literal descriptor, validated through its relocations."""

    def relocation(segment, at, kind):
        return _relocation(segment, at, kind, data, fixups)

    pointer = relocation(index, offset, omf.LOC_OFF16)
    selector = relocation(index, offset + 2, omf.LOC_OFF16)
    if pointer is None or selector is None or pointer.index not in far_strings or selector.index != index:
        return None
    base = relocation(index, selector.disp, omf.LOC_BASE)
    indirect = relocation(pointer.index, pointer.disp, omf.LOC_OFF16)
    if (
        base is None
        or base.index != pointer.index
        or base.disp != 0
        or indirect is None
        or indirect.index != pointer.index
        or indirect.disp != pointer.disp + 2
    ):
        return None
    length_at = indirect.disp
    length_bytes = [(pointer.index, length_at + byte) for byte in range(2)]
    if any(key not in data or key in ambiguous for key in length_bytes):
        return None
    length = int.from_bytes(bytes(data[key] for key in length_bytes), "little")
    if any(
        (pointer.index, byte) not in data or (pointer.index, byte) in ambiguous
        for byte in range(length_at + 2, length_at + 2 + length)
    ):
        return None
    return (*range(offset, offset + 4), *range(selector.disp, selector.disp + 2))


def _relocation(segment, at, kind, data, fixups):
    widths = {omf.LOC_OFF16: 2, omf.LOC_BASE: 2, omf.LOC_PTR32: 4}
    fields = [
        fixup
        for fixup in fixups
        if fixup.seg == segment and fixup.offset < at + 2 and at < fixup.offset + widths.get(fixup.loc, 4)
    ]
    if (
        len(fields) != 1
        or fields[0].offset != at
        or fields[0].loc != kind
        or fields[0].target != "segment"
        or any(data.get((segment, at + byte)) != 0 for byte in range(2))
    ):
        return None
    return fields[0]
