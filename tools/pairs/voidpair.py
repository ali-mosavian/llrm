#!/usr/bin/env python3
"""Whether two binaries are one build: a pair built into one shared CARGO_TARGET_DIR gave two llrm-c that differed by 83 path bytes, and the pair read 0 objects changed.

    tools/pairs/voidpair.py OLD NEW      prints `distinct: N bytes differ` and exits 0, or `VOID PAIR: ...` and exits 1

Distinct means: different sizes, or at least MIN_DIFF differing bytes (a rebuilt compiler differs in thousands; a relinked one in embedded paths).
"""
from __future__ import annotations

import sys
from pathlib import Path

MIN_DIFF = 4096


def differing(a: bytes, b: bytes) -> int:
    return sum(x != y for x, y in zip(a, b)) + abs(len(a) - len(b))


def void(old: str, new: str) -> str:
    """Why the pair is void, or ''."""
    a, b = Path(old).read_bytes(), Path(new).read_bytes()
    if len(a) != len(b):
        return ""
    count = differing(a, b)
    if count < MIN_DIFF:
        return f"VOID PAIR: {old} and {new} are {len(a)} bytes each and differ in {count} (< {MIN_DIFF}): one build, relinked; build each side in its own CARGO_TARGET_DIR"
    return ""


def main() -> int:
    why = void(sys.argv[1], sys.argv[2])
    print(why or "distinct")
    return 1 if why else 0


if __name__ == "__main__":
    sys.exit(main())
