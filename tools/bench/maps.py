"""Public symbols from a linker map: jwlink's (C, Nib) and Microsoft LINK's /MAP (BASIC)."""

import re
from pathlib import Path

SYMBOL = re.compile(r"^\s*([0-9A-Fa-f]{4}):([0-9A-Fa-f]{4})\S*\s+(\S+)\s*$")


def symbols(listing: Path) -> dict[str, int]:
    """Name to linear address from the image's first byte (frame * 16 + offset)."""
    found: dict[str, int] = {}
    for line in listing.read_text(errors="replace").splitlines():
        match = SYMBOL.match(line)
        if match:
            found.setdefault(match[3], int(match[1], 16) * 16 + int(match[2], 16))
    return found


def find(table: dict[str, int], name: str) -> int | None:
    """`name` as a language spells it: C and Nib prefix an underscore, BASIC upper-cases and may add a type character."""
    for spelling, address in table.items():
        plain = spelling.lstrip("_").rstrip("&%#!$").lower()
        if plain == name.lstrip("_").rstrip("&%#!$").lower():
            return address
    return None


SEGMENT = re.compile(r"^(\S+)\s+\S+\s+\S+\s+([0-9A-Fa-f]{4}):([0-9A-Fa-f]{4})\s+([0-9A-Fa-f]{8})\s*$")


def segments(listing: Path) -> list[tuple[str, int, int]]:
    """(name, linear start, size) of each segment in a jwlink map."""
    out = []
    for line in listing.read_text(errors="replace").splitlines():
        match = SEGMENT.match(line)
        if match:
            out.append((match[1], int(match[2], 16) * 16 + int(match[3], 16), int(match[4], 16)))
    return out


def locate(exe: Path, listing: Path, name: str) -> int | None:
    """The linear address of function `name`. Nib does not publish a function: the one it is, is the one
    `main` calls inside main's own code segment, when it makes exactly one such call."""
    table = symbols(listing)
    found = find(table, name)
    if found is not None:
        return found
    main = find(table, "main")
    own = [(start, size) for _, start, size in segments(listing) if main is not None and start <= main < start + size]
    if main is None or not own:
        return None
    from iced_x86 import Decoder, Mnemonic, OpKind

    data = exe.read_bytes()
    header = int.from_bytes(data[8:10], "little") * 16
    start, size = own[0]
    frame = main & ~0xFFFF
    inside = set()
    for one in Decoder(16, data[header + main : header + main + 512], ip=main & 0xFFFF):
        if one.mnemonic == Mnemonic.CALL:
            kind = one.op0_kind
            target = frame + one.near_branch16 if kind == OpKind.NEAR_BRANCH16 else one.far_branch_selector * 16 + one.far_branch16 if kind == OpKind.FAR_BRANCH16 else None
            if target is not None and start <= target < start + size:
                inside.add(target)
        if one.mnemonic in (Mnemonic.RET, Mnemonic.RETF):
            break
    return inside.pop() if len(inside) == 1 else None
