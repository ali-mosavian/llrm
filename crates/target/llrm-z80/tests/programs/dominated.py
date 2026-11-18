#!/usr/bin/env python3
"""Cells a higher level makes bigger and no faster than the same compiler's lower level: `dominated.py O1.tsv O2.tsv [O3.tsv]`.

A level that spends bytes must buy clocks (O2/O3 at 1 B = 3 T), so a cell with at least as many bytes and at least as many
clocks as the level below, and one of them more, is a pipeline defect, not a trade (idioms -O2 inlined a loop at 13 sites:
4555 B / 25.58M T against -O1's 1869 B / 24.47M T). Exit 1 for a cell not listed in `dominated_known.tsv` (cell, row id of
docs/working/above-main-rows.md), so a new one fails the sweep and an old one has an owner and a date.
"""
import csv
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
KNOWN = HERE / "dominated_known.tsv"


def load(path):
    """`{program: (bytes, clocks)}` of the `ours` rows."""
    rows = {}
    for row in csv.reader(open(path), delimiter="\t"):
        if len(row) >= 4 and row[1] == "ours" and row[2].isdigit():
            rows[row[0]] = (int(row[2]), int(row[3]))
    return rows


def dominated(low, high):
    """`[(program, extra bytes, extra clocks)]`: the cells of `high` no better than `low` on either axis and worse on one."""
    out = []
    for program in sorted(high):
        if program in low:
            (b1, t1), (b2, t2) = low[program], high[program]
            if b2 >= b1 and t2 >= t1 and (b2 > b1 or t2 > t1):
                out.append((program, b2 - b1, t2 - t1))
    return out


def known():
    """`{"O1>O2 program": row id}`."""
    if not KNOWN.exists():
        return {}
    return {" ".join(row[:1]): row[1] for row in csv.reader(open(KNOWN), delimiter="\t") if len(row) >= 2}


def main(paths):
    levels = [load(path) for path in paths]
    allowed, new = known(), []
    for at in range(len(levels) - 1):
        name = f"O{at + 1}>O{at + 2}"
        for program, bytes_, clocks in dominated(levels[at], levels[at + 1]):
            cell = f"{name} {program}"
            print(f"{cell}\t+{bytes_} B\t+{clocks} T\t{allowed.get(cell, 'NEW')}")
            if cell not in allowed:
                new.append(cell)
    sys.exit(1 if new else 0)


if __name__ == "__main__":
    main(sys.argv[1:])
