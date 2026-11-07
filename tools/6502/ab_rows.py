#!/usr/bin/env python3
"""Per-row A/B of two ours_cr tsv files (name, ours, bytes, clocks): worse rows first, then the table.
ab_rows.py BASE.tsv NEW.tsv [--allow-worse N]: exit 1 when more rows are worse than N (default 0).
A row is worse when its bytes or its clocks grew; the count is asserted, not only printed."""
import sys

def rows(path):
    out = {}
    for line in open(path):
        parts = line.split()
        if len(parts) == 4:
            out[parts[0]] = (int(parts[2]), int(parts[3]))
    return out

def compare(base, new):
    """(worse, better, same, missing): rows by name; a row only one side has is missing, never silently same."""
    worse, better, same = [], [], []
    for name in sorted(base.keys() & new.keys()):
        (bb, bc), (nb, nc) = base[name], new[name]
        (worse if nb > bb or nc > bc else better if nb < bb or nc < bc else same).append((name, base[name], new[name]))
    return worse, better, same, sorted(base.keys() ^ new.keys())

def main(argv):
    allowed = int(argv[argv.index("--allow-worse") + 1]) if "--allow-worse" in argv else 0
    worse, better, same, missing = compare(rows(argv[1]), rows(argv[2]))
    print(f"worse {len(worse)} better {len(better)} same {len(same)} missing {len(missing)}")
    for tag, group in (("WORSE", worse), ("better", better)):
        for name, (bb, bc), (nb, nc) in group:
            print(f"{tag:6} {name:34} {bb:6} -> {nb:6} B ({nb - bb:+d})  {bc:10} -> {nc:10} cy ({nc - bc:+d})")
    for name in missing:
        print(f"MISSING {name}")
    return 1 if len(worse) > allowed or missing else 0

if __name__ == "__main__":
    sys.exit(main(sys.argv))
