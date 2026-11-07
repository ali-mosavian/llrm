#!/usr/bin/env python3
"""Per-row A/B of two ours_cr tsv files (name, ours, bytes, clocks): failing rows first, then the table.
ab_rows.py BASE.tsv NEW.tsv --level O1|O2|O3|Os [--clocks mean|min] [--noise F] [--allow-worse N]: exit 1 when more rows fail than N (default 0).
clocks that grew by no more than the fraction F of the row (default 0, strict) are layout noise: the same binary moves ~0.5% between
the 16 pads, a mean of 16 layouts still moves 0.03% on a code shift of one byte.
The verdict is the level's rule, asserted and not only printed (oversight 2026-11-07):
  O1       strict: bytes and clocks both must not grow;
  Os       bytes only, clocks are free;
  O2, O3   3 clocks to the byte: bytes saved x 3 must cover the clocks added (bytes added cost 3 clocks each)."""
import sys

def rows(path, clocks="mean"):
    """name -> (bytes, clocks). "min" takes the fastest of the layouts in PATH.json (ours_cr writes it beside the tsv):
    the same binary moves ~0.5% between pads, the fastest layout is the one no page crossing touched."""
    out = {}
    for line in open(path):
        parts = line.split()
        if len(parts) == 4:
            out[parts[0]] = (int(parts[2]), int(parts[3]))
    if clocks == "min":
        import json
        for name, verdict, data in json.load(open(path + ".json")):
            if verdict == "PASS" and name in out:
                out[name] = (out[name][0], min(data["layouts"]))
    return out

CLOCKS_PER_BYTE = 3

def fails(level, base, new, noise=0.0):
    """Whether a row breaks the level's rule."""
    (bb, bc), (nb, nc) = base, new
    if nc > bc and nc - bc <= noise * bc:
        nc = bc
    if level == "O1":
        return nb > bb or nc > bc
    if level == "Os":
        return nb > bb
    return (nc - bc) + CLOCKS_PER_BYTE * (nb - bb) > 0

def compare(base, new, level="O1", noise=0.0):
    """(worse, better, same, missing): rows by name; a row only one side has is missing, never silently same.
    `worse` holds the rows that fail the level's rule; `better` those that do not and moved."""
    worse, better, same = [], [], []
    for name in sorted(base.keys() & new.keys()):
        row = (name, base[name], new[name])
        (worse if fails(level, base[name], new[name], noise) else better if base[name] != new[name] else same).append(row)
    return worse, better, same, sorted(base.keys() ^ new.keys())

def main(argv):
    allowed = int(argv[argv.index("--allow-worse") + 1]) if "--allow-worse" in argv else 0
    level = argv[argv.index("--level") + 1] if "--level" in argv else sys.exit("ab_rows: --level O1|O2|O3|Os is required")
    clocks = argv[argv.index("--clocks") + 1] if "--clocks" in argv else "mean"
    noise = float(argv[argv.index("--noise") + 1]) if "--noise" in argv else 0.0
    worse, better, same, missing = compare(rows(argv[1], clocks), rows(argv[2], clocks), level, noise)
    if missing:
        print(f"{level}: VOID, {len(missing)} rows are on one side only (refused, failed or not run): no verdict")
    print(f"{level}: failing {len(worse)} better {len(better)} same {len(same)} missing {len(missing)}")
    for tag, group in (("FAIL", worse), ("better", better)):
        for name, (bb, bc), (nb, nc) in group:
            print(f"{tag:6} {name:34} {bb:6} -> {nb:6} B ({nb - bb:+d})  {bc:10} -> {nc:10} cy ({nc - bc:+d})")
    for name in missing:
        print(f"MISSING {name}")
    return 1 if len(worse) > allowed or missing else 0

if __name__ == "__main__":
    sys.exit(main(sys.argv))
