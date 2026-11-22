#!/usr/bin/env python3
"""Instructions retired, base against new: `tools/clockpair.py BASE.json NEW.json` (files of `runsweep.py --clocks`).

Prints the total over the cells both counted, the share, and the five worst and best cells. Exit 1 when the total grew.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path


def compare(base: dict[str, int], new: dict[str, int]) -> tuple[int, int, list[tuple[int, str]]]:
    """(base total, new total, per-cell delta sorted ascending) over the cells in both."""
    common = sorted(set(base) & set(new))
    deltas = sorted((new[key] - base[key], key) for key in common)
    return sum(base[key] for key in common), sum(new[key] for key in common), deltas


def main(argv: list[str]) -> int:
    base, new = (json.loads(Path(path).read_text())["n"] for path in argv[:2])
    before, after, deltas = compare(base, new)
    share = (after - before) / before * 100 if before else 0.0
    print(f"{len(deltas)} cells: {before} -> {after} ({after - before:+d}, {share:+.4f}%)")
    print("worst", deltas[-5:][::-1])
    print("best ", deltas[:5])
    return 1 if after > before else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
