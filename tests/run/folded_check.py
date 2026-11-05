#!/usr/bin/env python3
"""Reject a corpus program the compiler folds to (nearly) a constant: it measures little code.

usage: folded_check.py MATRIX.tsv [MATRIX.tsv ...] [--allow NAME ...]

MATRIX is a target's matrix file (`target prog level compiler code_bytes ...`, `#` lines ignored) with one extra row
per level for the empty program (`prog` = `__empty__`, compiler `ours`: main and one call of `report`, same harness and
level), the target's own floor. A program is folded when, above that floor,
  - ours is at most 40 B at every level it was measured at, or
  - ours at -O2 is under a third of ours at -Os (the loop or the input was a constant at -O2 only).
The floor is per target (an object has a 12 B floor on the z80, 60-70 B on x86-64), so no absolute size is used. A
folded program is fixed by reading its inputs through a `volatile`; `folded.allow` (name, TAB, reason) lists the programs that are small by design, `--allow` names exempt until a fix lands.
Exit 1 and one line per folded program.
"""
import os
import sys

FLOOR = "__empty__"
FEW = 40


def folded(table):
    """table: {prog: {level: bytes}} of one target, with the floor row; the folded programs and why."""
    floor = table.get(FLOOR)
    if not floor:
        raise SystemExit("no `__empty__` row: the target's floor is not measured")
    out = {}
    for prog, levels in sorted(table.items()):
        if prog == FLOOR:
            continue
        missing = sorted(set(levels) - set(floor))
        if missing:
            raise SystemExit(f"no `__empty__` row at {', '.join(missing)} (needed for {prog}): the floor is not measured")
        above = {lv: size - floor[lv] for lv, size in levels.items()}
        if all(a <= FEW for a in above.values()):
            out[prog] = f"{max(above.values())} B above the floor at every level ({above})"
        elif "O2" in above and "Os" in above and above["O2"] * 3 < above["Os"]:
            out[prog] = f"-O2 {above['O2']} B above the floor against -Os {above['Os']} B"
    return out


def exempt(path):
    """The names in an allow file: `name<TAB>reason` lines, `#` lines ignored; a name without a reason is refused."""
    names = set()
    for line in open(path):
        if not line.strip() or line.startswith("#"):
            continue
        name, _, reason = line.rstrip("\n").partition("\t")
        if not reason.strip():
            raise SystemExit(f"{path}: `{name}` is exempt without a reason")
        names.add(name)
    return names


def read(path):
    tables = {}
    for line in open(path):
        if line.startswith("#") or line.startswith("target\t"):
            continue
        cells = line.rstrip("\n").split("\t")
        if len(cells) < 5 or cells[3] != "ours" or not cells[4].isdigit():
            continue
        target, prog, level, _, size = cells[:5]
        tables.setdefault(target, {}).setdefault(prog, {})[level] = int(size)
    return tables


def main(argv):
    here = os.path.join(os.path.dirname(os.path.abspath(__file__)), "folded.allow")
    allow = exempt(here) if os.path.exists(here) else set()
    if "--allow" in argv:
        at = argv.index("--allow")
        allow, argv = allow | set(argv[at + 1:]), argv[:at]
    bad = 0
    for path in argv:
        for target, table in read(path).items():
            for prog, why in folded(table).items():
                if prog not in allow:
                    print(f"{target} {prog}: folded, {why}")
                    bad += 1
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
