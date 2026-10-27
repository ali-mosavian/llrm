"""Bytes per procedure of an OMF object, and which procedures grew between two objects.

usage: procsizes.py OLD.obj NEW.obj [N]

A procedure is the distance from its PUBDEF offset to the next one in the same segment; the last public of a segment has no size.
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


def sizes(data: bytes) -> dict[str, int]:
    result = {}
    for entries in publics(data).values():
        entries.sort()
        result.update({name: after - offset for (offset, name), (after, _) in zip(entries, entries[1:])})
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
