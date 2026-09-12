from qbopt.objectfile import omf


def canonical(records: list[omf.Record], segment: int, size: int) -> list[omf.Record] | str:
    image = omf.segment_image(records, segment, size)
    edits: dict[int, list[tuple[int, int, bytes]]] = {}
    cleared: set[int] = set()
    for fixup in omf.fixups(records):
        if fixup.seg != segment or fixup.loc != omf.LOC_OFF16:
            continue
        at = fixup.offset
        if at + 2 > len(image):
            return "relocation addend extends beyond code segment"
        addend = int.from_bytes(image[at : at + 2], "little")
        if not addend:
            continue
        if fixup.selfrel or cleared.intersection((at, at + 1)):
            return "unsupported relative or overlapping relocation addend"
        displacement = (fixup.disp + addend) & 0xFFFF
        if fixup.disp_pos is not None:
            raw = omf.reemit(fixup, disp=displacement)
        else:
            # FIXDAT.P omits target displacement; clearing it adds a word
            # after the existing frame/target data, including threaded forms.
            raw = fixup.raw[:2] + bytes((fixup.raw[2] & ~4,)) + fixup.raw[3:] + displacement.to_bytes(2, "little")
        edits.setdefault(id(fixup.record), []).append((fixup.lo, fixup.hi, raw))
        cleared.update((at, at + 1))
    if not cleared:
        return records
    chunks = {id(record): (offset, payload) for record, seg, offset, payload in omf.ledata(records) if seg == segment}
    result = []
    for record in records:
        if id(record) in edits:
            end = 0
            pieces = []
            for lo, hi, raw in sorted(edits[id(record)]):
                pieces.extend((record.body[end:lo], raw))
                end = hi
            pieces.append(record.body[end:])
            result.append(omf.Record(record.type, b"".join(pieces)))
        elif id(record) in chunks:
            offset, payload = chunks[id(record)]
            changed = bytes(0 if offset + index in cleared else byte for index, byte in enumerate(payload))
            prefix = record.body[: len(record.body) - len(payload)]
            result.append(omf.Record(record.type, prefix + changed))
        else:
            result.append(record)
    return result
