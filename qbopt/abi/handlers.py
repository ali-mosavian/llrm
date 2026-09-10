"""Code entries passed to the runtime's established far-address registrations."""

from qbopt.objectfile import module


def registered(found: module.Module, routine: str, families: tuple[str, ...]) -> frozenset[int]:
    if module.family(found.records) not in families or routine in module.defines(found.records, found.seg):
        return frozenset()
    entries = set()
    for at, name in found.calls.items():
        if name != routine or at < 4:
            continue
        if found.code[max(0, at - 7):at] == b"\x0e\xb8\x00\x00\x68\x00\x00":
            field = at - 2
            if found.operands.get(at - 5) != found.operands.get(field):
                continue
        else:
            match found.code[max(0, at - 5):at]:
                case b"\xb8\x00\x00\x0e\x50":
                    field = at - 4
                case b"\x0e\xb8\x00\x00\x50":
                    field = at - 3
                case _:
                    if found.code[at - 4:at] != b"\x0e\x68\x00\x00":
                        continue
                    field = at - 2
        ref = found.operands.get(field)
        if (ref is not None and ref.space is module.Space.SEGMENT
            and ref.index == found.seg and found.start <= ref.disp < found.end):
            entries.add(ref.disp)
    return frozenset(entries)


def error_entries(found: module.Module) -> frozenset[int]:
    """B$OEGA consumes parmD erradr; rt/error.asm installs it as OFD_ONERROR."""
    return registered(found, "B$OEGA", ("qb45", "pds71", "vbdos"))
