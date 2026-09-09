"""Expose explicit BC literal initializer bytes at module entry.

BC_CN also contains descriptors and relocatable data. Only direct scalar
reads backed by complete, nonoverlapping, unrelocated bytes are admitted.
These are initial memory values, not immutable loads; subsequent writes and
calls still kill them. Procedure entries do not inherit loader state.
"""

from dataclasses import replace

from qbopt.frontend.blocks import ENTRY
from qbopt.model import mir
from qbopt.objectfile import omf
from qbopt.objectfile.module import Space


def initialized(body: mir.MirBody, found) -> mir.MirBody:
    if body.entry != ENTRY:
        return body
    pools = {index for index, segment in enumerate(omf.segments(found.records))
             if segment is not None and segment[0] == "BC_CN"
             and not omf.pubdef_names(found.records, index)}
    if not pools:
        return body
    fixups = omf.fixups(found.records)
    data, ambiguous = {}, set()
    unknown = set()
    for fixup in fixups:
        if fixup.seg not in pools:
            continue
        width = {omf.LOC_OFF16: 2, omf.LOC_BASE: 2, omf.LOC_PTR32: 4}.get(fixup.loc)
        if width is None:
            unknown.add(fixup.seg)
            continue
        ambiguous.update((fixup.seg, fixup.offset + byte) for byte in range(width))
    for _, index, start, payload in omf.ledata(found.records):
        if index not in pools or index in unknown:
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
                if (ref.addr is None or ref.addr.space is not Space.SEGMENT or ref.addr.index not in pools
                    or ref.base is not None or ref.segment is not None or ref.width not in (1, 2, 4, 8)):
                    continue
                keys = [(ref.addr.index, offset) for offset in range(ref.addr.disp, ref.addr.disp + ref.width)]
                if all(key in data and key not in ambiguous for key in keys):
                    values[ref] = mir.Const(int.from_bytes(bytes(data[key] for key in keys), "little"), ref.width)
    return replace(body, initial=tuple(values.items())) if values else body
