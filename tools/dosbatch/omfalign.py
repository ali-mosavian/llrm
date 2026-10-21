"""OMF objects laid out for `jwlink format elf`, which ignores a segment's alignment.

The image that the emulator prices (tools/bench/flatcount.py) is linked from copies of the objects:

* A data segment is at most a dword aligned. `jwlink format elf` (1.9beta 13) splits a paragraph-aligned one into a loadable segment of its
  own and writes the section tables over the program (frames at bq32: tables at 0x102f, code to 0x19f5). #2407 aligned globals as
  getGVAlignment does, which gave the QB runtime's CONST2, _DATA and _BSS paragraph alignment.
* Each segment is a multiple of a dword long. The ELF writer puts one object's contribution to a segment right after the last one's, whatever
  alignment the object asks, so a 265-byte _BSS left the next object's variables at odd addresses: a string descriptor at 0x804e48f, which
  the runtime's string space reads as a free entry (its low bit is the free flag), and stft_fixed's string compaction walked into it.

The DOS32A executable is linked from the objects as they are. The price table (vsgcc's) has no alignment term.
"""

from __future__ import annotations

import struct
from pathlib import Path

SEGDEF, SEGDEF32, LNAMES = 0x98, 0x99, 0x96
PARAGRAPH, PAGE, DWORD, PAGE_4K = 3, 4, 5, 6  # the SEGDEF alignment field's values wider than a dword
WIDER = (PARAGRAPH, PAGE, PAGE_4K)
DWORD_BYTES = 4


def _segments(data: bytes) -> list[tuple[str, int, int, int, bool]]:
    """(class name, offset of the attribute byte, offset of the length, length, 32-bit) of each relocatable, not-big SEGDEF."""
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
        elif kind in (SEGDEF, SEGDEF32) and data[body] >> 5 != 0 and not data[body] & 2:
            wide = kind == SEGDEF32
            size_at = body + 1
            name_at = size_at + (4 if wide else 2)
            (size,) = struct.unpack_from("<I" if wide else "<H", data, size_at)
            out.append((names[data[name_at + 1] - 1], body, size_at, size, wide))
        at += 3 + length
    return out


def dword_laid_out(objects: list[Path], into: Path) -> list[Path]:
    """Copies of `objects` in the directory `into`, in order: each data segment (any class but CODE) at a dword where it was wider, and each
    segment's length a multiple of a dword."""
    into.mkdir(parents=True, exist_ok=True)
    out = []
    for number, one in enumerate(objects):
        data = bytearray(one.read_bytes())
        for class_, attribute_at, size_at, size, wide in _segments(bytes(data)):
            if class_ != "CODE" and data[attribute_at] >> 5 in WIDER:
                data[attribute_at] = (data[attribute_at] & 0x1F) | (DWORD << 5)
            rounded = -(-size // DWORD_BYTES) * DWORD_BYTES
            if rounded != size and (wide or rounded < 1 << 16):
                struct.pack_into("<I" if wide else "<H", data, size_at, rounded)
        copy = into / f"{number:03d}-{one.name}"
        copy.write_bytes(bytes(data))
        out.append(copy)
    return out
