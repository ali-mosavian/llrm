"""Bytes of code and of data in an OMF object: the LEDATA records by the class of their segment.

    omfcode.py OBJ...                 prints `code data` per object

`m16_bytes.py` counts every LEDATA byte, so a module's initialised data reads as code against the pin's `.text`.
"""
from __future__ import annotations

import sys


def _index(body: bytes, at: int) -> tuple[int, int]:
    first = body[at]
    return (first, 1) if first < 0x80 else (((first & 0x7F) << 8) | body[at + 1], 2)


def _ledata(data: bytes):
    """`(is_code, offset, payload)` of each LEDATA record (16- and 32-bit; a segment of class CODE or named *TEXT is code)."""
    names: list[str] = []
    segments: list[tuple[str, str]] = []
    at = 0
    while at < len(data):
        kind, size = data[at], data[at + 1] | data[at + 2] << 8
        body = data[at + 3 : at + 3 + size - 1]
        if kind == 0x96:
            i = 0
            while i < len(body):
                names.append(body[i + 1 : i + 1 + body[i]].decode("latin1"))
                i += 1 + body[i]
        elif kind in (0x98, 0x99):
            i = 1 + (4 if kind == 0x99 else 2)
            if kind == 0x98 and body[0] >> 5 == 0:
                i += 3  # an absolute segment names its frame and offset
            name, step = _index(body, i)
            klass, _ = _index(body, i + step)
            segments.append((names[name - 1], names[klass - 1] if klass else ""))
        elif kind in (0xA0, 0xA1):
            segment, step = _index(body, 0)
            width = 4 if kind == 0xA1 else 2
            name, klass = segments[segment - 1]
            yield "CODE" in klass.upper() or name.upper().endswith("TEXT"), int.from_bytes(body[step : step + width], "little"), body[step + width :]
        at += 3 + size


def code_bytes_of(data: bytes) -> tuple[int, int]:
    """`(code, data)` bytes of the LEDATA records of an OMF object."""
    code = other = 0
    for is_code, _, payload in _ledata(data):
        if is_code:
            code += len(payload)
        else:
            other += len(payload)
    return code, other


def code_blob_of(data: bytes) -> bytes:
    """The code segments' bytes, in offset order."""
    return b"".join(payload for _, _, payload in sorted((r for r in _ledata(data) if r[0]), key=lambda r: r[1]))


def code_bytes(path: str) -> tuple[int, int]:
    with open(path, "rb") as handle:
        return code_bytes_of(handle.read())


if __name__ == "__main__":
    for path in sys.argv[1:]:
        print(*code_bytes(path), path)
