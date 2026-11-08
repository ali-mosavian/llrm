#!/usr/bin/env python3
"""worse_cells.py MATRIX.tsv: the cells where ours is bigger than the best of gcc and clang (code bytes), from a matrix64 table.

A cell with no number for ours, gcc or clang is refused (exit 2, listed), never compared against the next-best column: a missing
reference understated the z80 gaps by up to 1.7 kB."""
import sys
from collections import defaultdict

COLUMNS = ("ours", "gcc", "clang")


def cells(lines: list[str]) -> dict[tuple[str, str], dict[str, str]]:
    found: dict[tuple[str, str], dict[str, str]] = defaultdict(dict)
    for line in lines:
        f = line.rstrip("\n").split("\t")
        if len(f) >= 5 and not line.startswith(("#", "target")) and f[3] in COLUMNS:
            found[(f[1], f[2])][f[3]] = f[4]
    return found


def worse(table: dict[tuple[str, str], dict[str, str]]) -> tuple[list[tuple[str, str, int, int]], list[str]]:
    missing, rows = [], []
    for (program, level), row in sorted(table.items()):
        gone = [c for c in COLUMNS if not row.get(c, "").isdigit()]
        if gone:
            missing.append(f"{program} {level}: no {', '.join(gone)}")
            continue
        best = min(int(row["gcc"]), int(row["clang"]))
        if int(row["ours"]) > best:
            rows.append((program, level, int(row["ours"]), best))
    return rows, missing


if __name__ == "__main__":
    rows, missing = worse(cells(open(sys.argv[1]).read().splitlines()))
    for program, level, ours, best in sorted(rows, key=lambda r: -(r[2] - r[3]) / r[3]):
        print(f"{program}\t{level}\t{ours}\t{best}\t{100 * (ours - best) / best:+.1f}%")
    for line in missing:
        print("MISSING", line, file=sys.stderr)
    sys.exit(2 if missing else 0)
