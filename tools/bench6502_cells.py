"""Compare two 6502 bench outputs cell by cell (`ours` columns), n/a counts as a failure.

usage: bench6502_cells.py BASE.txt NEW.txt

A cell that is numeric in BASE and n/a (or gone) in NEW is listed first and the exit status is 1: a build
that stopped assembling is not "unchanged", and a sum over the cells both runs have would hide it.
"""
import re
import sys

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


def main(argv):
    base, new = rows(argv[1]), rows(argv[2])
    broken, both = compare(base, new)
    for key in broken:
        print("BROKEN", key[0], key[1], "was", base[key])
    total = lambda d, ks, i: sum(d[k][i] for k in ks)
    print(
        f"compared {len(both)} of {len(both) + len(broken)} cells: bytes {total(new, both, 0)} (base {total(base, both, 0)}), "
        f"cycles {total(new, both, 1)} (base {total(base, both, 1)})"
    )
    worse = sorted(((new[k][0] - base[k][0], new[k][1] - base[k][1], k[0], k[1]) for k in both if new[k][0] > base[k][0]), reverse=True)
    for row in worse[:15]:
        print("worse", row)
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
