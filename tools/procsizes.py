"""Bytes per procedure of an OMF object, and which procedures grew between two objects.

usage: procsizes.py OLD.obj NEW.obj [N]

A procedure is the distance from its PUBDEF offset to the next one in the same segment; the last public of a segment runs to the end of the segment (its SEGDEF length).
"""

import struct
import sys
from pathlib import Path


def index(body: bytes, at: int) -> tuple[int, int]:
    if body[at] & 0x80:
        return ((body[at] & 0x7F) << 8) | body[at + 1], at + 2
    return body[at], at + 1


def publics(data: bytes) -> dict[int, list[tuple[int, str]]]:
    """(offset, name) of every public, by segment index."""
    found: dict[int, list[tuple[int, str]]] = {}
    at = 0
    while at < len(data):
        kind, length = data[at], struct.unpack("<H", data[at + 1 : at + 3])[0]
        body = data[at + 3 : at + 3 + length - 1]
        at += 3 + length
        if kind not in (0x90, 0x91):
            continue
        wide = kind & 1
        _, i = index(body, 0)
        segment, i = index(body, i)
        if segment == 0:
            i += 2
        while i < len(body):
            name = body[i + 1 : i + 1 + body[i]].decode("latin1")
            i += 1 + body[i]
            size = 4 if wide else 2
            offset = int.from_bytes(body[i : i + size], "little")
            _, i = index(body, i + size)
            found.setdefault(segment, []).append((offset, name))
    return found


def lengths(data: bytes) -> list[int]:
    """The length of each segment, in SEGDEF order (segment indexes count from 1)."""
    found = []
    at = 0
    while at < len(data):
        kind, length = data[at], struct.unpack("<H", data[at + 1 : at + 3])[0]
        body = data[at + 3 : at + 3 + length - 1]
        at += 3 + length
        if kind not in (0x98, 0x99):
            continue
        attributes, i = body[0], 1
        if attributes >> 5 == 0:
            i += 3
        size = 4 if kind & 1 else 2
        found.append(int.from_bytes(body[i : i + size], "little") or (1 << (8 * size) if attributes & 2 else 0))
    return found


def sizes(data: bytes) -> dict[str, int]:
    result = {}
    ends = lengths(data)
    for segment, entries in publics(data).items():
        entries.sort()
        end = ends[segment - 1] if 0 < segment <= len(ends) else None
        following = [offset for offset, _ in entries[1:]] + ([end] if end is not None else [])
        result.update({name: after - offset for (offset, name), after in zip(entries, following)})
    return result


def growth(old: dict[str, int], new: dict[str, int]) -> list[tuple[str, int, int]]:
    """(name, old, new) of each procedure in both that changed, largest growth first."""
    return sorted(((n, old[n], new[n]) for n in new if n in old and old[n] != new[n]), key=lambda row: row[2] - row[1], reverse=True)


def main() -> None:
    old, new = (sizes(Path(path).read_bytes()) for path in sys.argv[1:3])
    rows = growth(old, new)
    print(f"total {sum(n - o for _, o, n in rows):+d} B over {len(rows)} procedures")
    for name, before, after in rows[: int(sys.argv[3]) if len(sys.argv) > 3 else 15]:
        print(f"{after - before:+6d} {name} ({before} -> {after})")


if __name__ == "__main__":
    main()
