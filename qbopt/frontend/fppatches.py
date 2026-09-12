from dataclasses import replace

from iced_x86 import Code

from qbopt.objectfile import omf
from qbopt.frontend.declen import decode
from qbopt.objectfile.module import Module


def native_records(module: Module, starts: frozenset[int]) -> list[omf.Record]:
    patches = sites(module, starts)
    removed: dict[int, list[tuple[int, int]]] = {}
    for fixup in omf.fixups(module.records):
        if fixup.seg == module.seg and fixup.offset in patches:
            removed.setdefault(id(fixup.record), []).append((fixup.lo, fixup.hi))
    records: list[omf.Record] = []
    for record in module.records:
        ranges = sorted(removed.get(id(record), ()))
        if not ranges:
            records.append(record)
            continue
        fragments = []
        cursor = 0
        for lo, hi in ranges:
            fragments.append(record.body[cursor:lo])
            cursor = hi
        fragments.append(record.body[cursor:])
        body = b"".join(fragments)
        if body:
            records.append(replace(record, body=body, raw=None))
    return records


def sites(module: Module, starts: frozenset[int]) -> frozenset[int]:
    names = omf.externals(module.records)
    result: set[int] = set()
    overrides = {"FIERQQ": 0x26, "FICRQQ": 0x2E, "FISRQQ": 0x36, "FIARQQ": 0x3E}
    for at, fixup in module.fixup_at.items():
        if (
            at not in starts
            or fixup.loc != omf.LOC_OFF16
            or fixup.selfrel
            or fixup.target != "external"
            or fixup.disp != 0
            or not 0 < fixup.index < len(names)
        ):
            continue
        name = names[fixup.index]
        if name == "FIWRQQ":
            if module.code[at : at + 2] == b"\x90\x9b" and at + 1 in starts:
                result.add(at)
            continue
        opcode = at + 1
        if name in overrides:
            if module.code[opcode : opcode + 1] != bytes([overrides[name]]):
                continue
            opcode += 1
        elif name != "FIDRQQ":
            continue
        if (
            module.code[at : at + 1] != b"\x9b"
            or at + 1 not in starts
            or opcode >= module.end
            or not 0xD8 <= module.code[opcode] <= 0xDF
        ):
            continue
        instruction = decode(module.code, at + 1)
        if instruction is not None and instruction.code != Code.INVALID:
            result.add(at)
    return frozenset(result)
