"""A/B of two tools/arm64/cell.sh outputs: cellcmp.py BASE.tsv NEW.tsv.

Rows are cell.sh's: prog level comp bytes instrs metric value flags. A pair is
compared only if both rows have the same column (`comp`), metric and known
flags; otherwise the pair is refused and the exit status is 1. A `cc` row
against a `ccn` row differs by flags, not by the change under test."""

import sys


def read(path):
    rows = {}
    for line in open(path):
        f = line.rstrip("\n").split("\t")
        if len(f) >= 8:
            rows[(f[0], f[1])] = f
    return rows


def compare(base, new):
    """(lines, refused): one line per cell in both files."""
    lines, refused = [], False
    for key in sorted(base.keys() & new.keys()):
        b, n = base[key], new[key]
        name = f"{key[0]} O{key[1]}"
        why = []
        if b[2] != n[2]:
            why.append(f"column {b[2]} vs {n[2]}")
        if b[5] != n[5]:
            why.append(f"metric {b[5]} vs {n[5]}")
        if b[7] != n[7] or "unknown" in (b[7], n[7]):
            why.append(f"flags {b[7]!r} vs {n[7]!r}")
        if not b[4].isdigit() or not n[4].isdigit():
            why.append("unmeasured")
        if why:
            refused = True
            lines.append(f"REFUSED {name}: {', '.join(why)}")
            continue
        db, di = int(n[3]) - int(b[3]), int(n[4]) - int(b[4])
        lines.append(f"{name} [{b[2]}] bytes {b[3]} -> {n[3]} ({db:+d}), instrs {b[4]} -> {n[4]} ({di:+d})")
    return lines, refused


if __name__ == "__main__":
    out, bad = compare(read(sys.argv[1]), read(sys.argv[2]))
    print("\n".join(out))
    sys.exit(1 if bad else 0)
