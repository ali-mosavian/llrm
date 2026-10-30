#!/usr/bin/env python3
"""The rows two builds changed, from two `rows/run.sh` result sets.

    tools/pairs/rowdiff.py A B [--programs p1,p2]      A, B: names under ROWS_WORK (default ~/scratch/rows), each with results.jsonl

Per row (program, level) the llrm clocks and code bytes A -> B, and the count of rows above A. A row present on one side only is named, not skipped.
Exit 1 when a row is above A.
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

LEVELS = {"O1": "llrmO1", "O2": "llrm", "O3": "llrmO3", "Os": "llrmOs"}


def load(path: Path) -> dict[tuple[str, str], dict]:
    rows = {}
    for line in path.read_text().splitlines():
        if line.startswith("{"):
            row = json.loads(line)
            rows[(row["prog"], row["variant"])] = row
    return rows


def diff(a: dict, b: dict, programs: set[str]) -> tuple[list[str], int]:
    lines, above = [], 0
    for prog in sorted({p for p, _ in a} | {p for p, _ in b}):
        if programs and prog not in programs:
            continue
        for level, variant in LEVELS.items():
            x, y = a.get((prog, variant)), b.get((prog, variant))
            if x is None or y is None:
                lines.append(f"{prog} -{level}: only in {'A' if y is None else 'B'}")
                continue
            cells = []
            for counter in ("clocks", "code"):
                p, q = x[counter], y[counter]
                above += q > p
                cells.append(f"{counter} {p}->{q} ({(q / max(p, 1) - 1) * 100:+.1f}%)")
            lines.append(f"{prog} -{level}: " + ", ".join(cells))
    return lines, above


def main() -> int:
    work = Path(os.environ.get("ROWS_WORK", Path.home() / "scratch/rows"))
    programs = set(sys.argv[sys.argv.index("--programs") + 1].split(",")) if "--programs" in sys.argv else set()
    lines, above = diff(load(work / sys.argv[1] / "results.jsonl"), load(work / sys.argv[2] / "results.jsonl"), programs)
    print("\n".join(lines))
    print(f"{above} counters above A")
    return 1 if above else 0


if __name__ == "__main__":
    sys.exit(main())
