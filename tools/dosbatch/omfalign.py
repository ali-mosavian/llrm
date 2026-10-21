"""OMF objects whose data segments ask no more than a dword of alignment.

`jwlink format elf` (1.9beta 13) lays out an image wrongly when a data segment is paragraph-aligned: it starts a loadable segment for it
and a page's gap before the next, and writes the section tables over the program (frames at bq32: tables at 0x102f, code to 0x19f5, so
the emulator ran relocation records). #2407 aligned globals as getGVAlignment does, which gave the QB runtime's CONST2, _DATA and _BSS
paragraph alignment in some objects. The image that the emulator prices is linked from copies of the objects with those at a dword; the
DOS32A executable is linked from the objects as they are. The price table (vsgcc's) has no alignment term, so no count moves.
"""

from __future__ import annotations

import struct
from pathlib import Path

SEGDEF, SEGDEF32, LNAMES = 0x98, 0x99, 0x96
PARAGRAPH, PAGE, DWORD, PAGE_4K = 3, 4, 5, 6  # the SEGDEF alignment field's values wider than a dword
WIDER = (PARAGRAPH, PAGE, PAGE_4K)


def _segments(data: bytes) -> list[tuple[str, str, int]]:
    """(segment name, class name, offset of the attribute byte) of each relocatable SEGDEF that is wider than a dword."""
    names: list[str] = []
    out = []
    at = 0
    while at < len(data):
        kind, length = data[at], struct.unpack_from("<H", data, at + 1)[0]
        body = at + 3
        if kind == LNAMES:
            end = body + length - 1
            while body < end:
                size = data[body]
                names.append(data[body + 1 : body + 1 + size].decode("latin1"))
                body += 1 + size
        elif kind in (SEGDEF, SEGDEF32) and data[body] >> 5 in WIDER:
            name_at = body + 1 + (4 if kind == SEGDEF32 else 2)
            out.append((names[data[name_at] - 1], names[data[name_at + 1] - 1], body))
        at += 3 + length
    return out


def at_most_a_dword(objects: list[Path], into: Path) -> list[Path]:
    """Copies of `objects` in the directory `into`, in order, each data segment (any class but CODE) at a dword where it was wider."""
    into.mkdir(parents=True, exist_ok=True)
    out = []
    for number, one in enumerate(objects):
        data = bytearray(one.read_bytes())
        for _, class_, at in _segments(bytes(data)):
            if class_ != "CODE":
                data[at] = (data[at] & 0x1F) | (DWORD << 5)
        copy = into / f"{number:03d}-{one.name}"
        copy.write_bytes(bytes(data))
        out.append(copy)
    return out
