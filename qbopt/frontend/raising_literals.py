"""Expose explicit BC literal initializer bytes at module entry.

BC_CN also contains descriptors and relocatable data. Only direct floating
reads backed by complete, nonoverlapping, unrelocated LEDATA are admitted.
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
    for _, index, start, payload in omf.ledata(found.records):
        if index not in pools:
            continue
        relocated = any(fixup.seg == index and start <= fixup.offset < start + len(payload) for fixup in fixups)
        for offset, byte in enumerate(payload, start):
            key = index, offset
            if key in data or relocated:
                ambiguous.add(key)
            data[key] = byte
    values = {}
    for block in body.blocks:
        for op in block.ops:
            if op.floating is None:
                continue
            for ref in op.loads:
                ref = mir._symbolic_ref(ref)
                if (ref.addr is None or ref.addr.space is not Space.SEGMENT or ref.addr.index not in pools
                    or ref.base is not None or ref.segment is not None or ref.width not in (2, 4, 8)):
                    continue
                keys = [(ref.addr.index, offset) for offset in range(ref.addr.disp, ref.addr.disp + ref.width)]
                if all(key in data and key not in ambiguous for key in keys):
                    values[ref] = mir.Const(int.from_bytes(bytes(data[key] for key in keys), "little"), ref.width)
    return replace(body, initial=tuple(values.items())) if values else body
