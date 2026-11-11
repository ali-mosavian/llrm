#!/usr/bin/env python3
"""6502 owner table: ours against the best of llvm-mos, cc65 and SDCC per cell.

    owner_table.py MATRIX.tsv OUT.tsv --corpus tests/run/c [--programs-no-bar]

A cell needs all three references. One that lacks any is refused (exit 2) and
listed: taking the best of the columns that happen to be present understated
gaps (z80, up to 1.7 kB), and a cell with none was skipped without a line
(6502: 172 program cells, so "125 worse of 324" was kernels only).
`--programs-no-bar` states that the corpus programs have no reference columns:
their cells are written as NO_BAR and counted, never as OK.
`--ungraded-cpus 65c02` states that a CPU's cells lack a reference column (the
matrix has no cc65 65C02 column): they are written as UNGRADED and counted.
Coverage: the corpus programs (`NAME.out` beside `NAME.c`) against the programs
the matrix has `ours` for, printed as in / missing / the missing names.
Os is strict bytes; O2 is clocks + 3 * bytes.
"""
import collections
import os
import sys

REQUIRED = ("llvm-mos", "cc65", "sdcc")


def cells_of(rows):
    """{(program, level, cpu): {column: (bytes, clocks)}} from matrix rows."""
    cells = collections.defaultdict(dict)
    for r in rows:
        r = list(r) + [""] * (8 - len(r))
        _, prog, lv, comp, b, c = r[:6]
        if not b.isdigit() or not c.strip().replace(".", "").isdigit():
            continue
        cells[(prog, lv, "65c02" if comp.endswith("65c02") else "6502")][comp.replace("-65c02", "")] = (int(b), float(c))
    return cells


def grade(cells, programs, programs_no_bar, ungraded_cpus=()):
    """(rows, refused): a row per graded, NO_BAR or UNGRADED cell; the cells refused."""
    rows, refused = [], []
    for (prog, lv, cpu), d in sorted(cells.items()):
        if "ours" not in d:
            continue
        absent = [c for c in REQUIRED if c not in d]
        ob, oc = d["ours"]
        if absent == list(REQUIRED) and prog in programs and programs_no_bar:
            rows.append((prog, lv, cpu, ob, oc, "", "", "NO_BAR"))
            continue
        if absent and cpu in ungraded_cpus:
            rows.append((prog, lv, cpu, ob, oc, "", "", "UNGRADED"))
            continue
        if absent:
            refused.append((prog, lv, cpu, absent))
            continue
        ext = [d[c] for c in REQUIRED]
        bb, bc = min(v[0] for v in ext), min(v[1] for v in ext)
        ok = ob <= bb if lv == "Os" else oc + 3 * ob <= min(v[1] + 3 * v[0] for v in ext)
        rows.append((prog, lv, cpu, ob, oc, bb, bc, "OK" if ok else "WORSE"))
    return rows, refused


def corpus_programs(corpus):
    return {f[:-4] for f in os.listdir(corpus) if f.endswith(".out") and os.path.exists(os.path.join(corpus, f[:-4] + ".c"))}


def coverage(cells, corpus_names):
    """(in, missing names): corpus programs the matrix has `ours` for, and not."""
    have = {prog for (prog, _, _), d in cells.items() if "ours" in d}
    return len(corpus_names & have), sorted(corpus_names - have)


def main(argv):
    args = [a for a in argv if not a.startswith("--")]
    corpus = argv[argv.index("--corpus") + 1] if "--corpus" in argv else None
    ungraded = argv[argv.index("--ungraded-cpus") + 1].split(",") if "--ungraded-cpus" in argv else []
    for value in (corpus, *ungraded):
        if value in args:
            args.remove(value)
    matrix, out = args[:2]
    rows = [l.rstrip("\n").split("\t") for l in open(matrix) if not l.startswith("#")][1:]
    cells = cells_of(rows)
    names = corpus_programs(corpus) if corpus else set()
    graded, refused = grade(cells, names, "--programs-no-bar" in argv, tuple(ungraded))
    inside, missing = coverage(cells, names) if corpus else (0, [])
    if corpus:
        print(f"coverage: {inside} corpus programs in the table, {len(missing)} missing")
        print("missing:", " ".join(missing))
    tally = collections.Counter(r[-1] for r in graded)
    print("cells:", dict(tally), f"refused {len(refused)}")
    if refused:
        for prog, lv, cpu, absent in refused:
            print(f"REFUSED {prog} {lv} {cpu}: no {', '.join(absent)}", file=sys.stderr)
        return 2
    header = "prog\tlevel\tcpu\tours_B\tours_clk\tbest_ext_B\tbest_ext_clk\tverdict\n"
    open(out, "w").write(header + "".join("\t".join(map(str, r)) + "\n" for r in graded))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
