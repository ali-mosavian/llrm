"""Compare two matrix TSVs (compiler `ours`): list cells worse, better or changed.

    worse.py BEFORE.tsv AFTER.tsv

A cell is worse when ANY of bytes, clocks or instrs grew. Exit 1 if any is.
"""
import csv
import sys

METRICS = ("code_bytes", "clocks", "instrs")


def cells(path):
    with open(path) as f:
        rows = csv.DictReader((l for l in f if not l.startswith("#")), delimiter="\t")
        return {(r["prog"], r["level"]): tuple(int(r[m]) if r[m] else None for m in METRICS) for r in rows if r["compiler"] == "ours"}


def compare(before, after):
    """(worse, better, mixed) lists of (cell, before, after) over the cells both files have."""
    worse, better, mixed = [], [], []
    for cell in sorted(before.keys() & after.keys()):
        b, a = before[cell], after[cell]
        if None in b or None in a or a == b:
            continue
        up = any(x > y for x, y in zip(a, b))
        down = any(x < y for x, y in zip(a, b))
        (mixed if up and down else worse if up else better).append((cell, b, a))
    return worse, better, mixed


if __name__ == "__main__":
    worse, better, mixed = compare(cells(sys.argv[1]), cells(sys.argv[2]))
    for tag, rows in (("WORSE", worse), ("MIXED", mixed), ("better", better)):
        for cell, b, a in rows:
            print(f"{tag} {cell[0]} {cell[1]}: {'/'.join(map(str, b))} -> {'/'.join(map(str, a))}")
    print(f"{len(worse)} worse, {len(mixed)} mixed, {len(better)} better")
    sys.exit(1 if worse or mixed else 0)
