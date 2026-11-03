"""Compare two 6502 bench outputs cell by cell (`ours` columns), n/a counts as a failure.

usage: bench6502_cells.py BASE.txt NEW.txt [--bin BASE_BINARY NEW_BINARY]...

The header says how many `ours` cells compiled in each leg, and with `--bin` (repeatable: llrm-cc is a thin driver, name the llc too) the md5 of each pair.
If every pair is one binary the A/B measures nothing and the exit is 1. A leg with no compiled
cell is a failure (exit 1): two logs of `n/a (llrm-cc: No such file)` once compared as "0 of 0 cells" and looked unchanged.

A cell that is numeric in BASE and n/a (or gone) in NEW is listed first and the exit status is 1: a build
that stopped assembling is not "unchanged", and a sum over the cells both runs have would hide it.
"""
import re
import sys
from pathlib import Path

CELL = re.compile(r"(\d+) B(?: linked)?(?:, \d+ B body)?, (\d+) cy")


def rows(path):
    out, header = {}, None
    for line in open(path):
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if cells[0] == "kernel":
            header = cells
        elif header and not set(cells[0]) <= {"-"} and len(cells) == len(header):
            for name, cell in zip(header[1:], cells[1:]):
                if name.startswith("ours"):
                    found = CELL.match(cell)
                    out[(cells[0], name)] = (int(found[1]), int(found[2])) if found else None
    return out


def compare(base, new):
    """(broken cells, compared pairs): a broken cell is numeric in `base` and not in `new`."""
    broken = sorted(k for k, v in base.items() if v and not new.get(k))
    both = sorted(k for k, v in base.items() if v and new.get(k))
    return broken, both


def md5(path):
    import hashlib

    return hashlib.md5(Path(path).read_bytes()).hexdigest()


def main(argv):
    base, new = rows(argv[1]), rows(argv[2])
    broken, both = compare(base, new)
    compiled = lambda d: sum(1 for v in d.values() if v)
    print(f"base: {compiled(base)} of {len(base)} ours cells compiled; new: {compiled(new)} of {len(new)}")
    pairs = [(argv[i + 1], argv[i + 2]) for i, a in enumerate(argv) if a == "--bin"]
    for old, fresh in pairs:
        print(f"{Path(fresh).name} md5 base {md5(old)[:8]} new {md5(fresh)[:8]}")
    if pairs and all(md5(old) == md5(fresh) for old, fresh in pairs):
        print("FAIL: every binary pair is identical; the legs ran one compiler")
        return 1
    if not compiled(base) or not compiled(new):
        print("FAIL: a leg compiled no ours cell; the comparison says nothing")
        return 1
    for key in broken:
        print("BROKEN", key[0], key[1], "was", base[key])
    total = lambda d, ks, i: sum(d[k][i] for k in ks)
    print(
        f"compared {len(both)} of {len(both) + len(broken)} cells: bytes {total(new, both, 0)} (base {total(base, both, 0)}), "
        f"cycles {total(new, both, 1)} (base {total(base, both, 1)})"
    )
    worse = sorted(((new[k][0] - base[k][0], new[k][1] - base[k][1], k[0], k[1]) for k in both if new[k][0] > base[k][0] or new[k][1] > base[k][1]), reverse=True)
    for row in worse[:15]:
        print("worse", row)
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
