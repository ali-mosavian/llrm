#!/usr/bin/env python3
"""6502 owner table: ours against the best of llvm-mos, cc65 and SDCC per cell.

    owner_table.py MATRIX.tsv OUT.tsv --corpus tests/run/c [--programs-no-bar]

A cell needs every reference: llvm-mos, cc65 and the SDCC variants of its level
(Os: size, size-ma; O2: size, speed, size-ma, speed-ma; a plain `sdcc` column is
SDCC's default optimisation and is not a reference). One that lacks any is refused (exit 2) and
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
VIRTUAL_OK (oversight ruling 2026-11-09): the per-axis bar (best bytes, best clocks, possibly from
different compilers) stays the reported target. A cell where NO single reference attains both axes is
virtual: it passes when ours is Pareto-optimal among the references (none is at least as good on both
axes and better on one) and is no worse than the pin-equivalent on both axes (as fputable.verdict, #8509). The pin
(LLVM d1106deb71) has no MOS target, so the pin-equivalent is the llvm-mos column, the nearest
reference: an assumption of this tool, stated here and in OUT.virtual.tsv's header.
"""
import collections
import os
import sys

REQUIRED = ("llvm-mos", "cc65")
# SDCC is never quoted at its default optimisation: the Os bar (strict bytes)
# comes from the size variants, the O2 bar from the best of all four.
SDCC_VARIANTS = {"Os": ("sdcc-size", "sdcc-size-ma"), "O2": ("sdcc-size", "sdcc-speed", "sdcc-size-ma", "sdcc-speed-ma")}


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
        sdcc = SDCC_VARIANTS.get(lv, SDCC_VARIANTS["O2"])
        absent = [c for c in REQUIRED + sdcc if c not in d]
        ob, oc = d["ours"]
        if len(absent) == len(REQUIRED + sdcc) and prog in programs and programs_no_bar:
            rows.append((prog, lv, cpu, ob, oc, "", "", "NO_BAR"))
            continue
        if absent and cpu in ungraded_cpus:
            rows.append((prog, lv, cpu, ob, oc, "", "", "UNGRADED"))
            continue
        if absent:
            refused.append((prog, lv, cpu, absent))
            continue
        ext = [d[c] for c in REQUIRED + sdcc]
        bb, bc = min(v[0] for v in ext), min(v[1] for v in ext)
        ok = ob <= bb if lv == "Os" else oc + 3 * ob <= min(v[1] + 3 * v[0] for v in ext)
        verdict = "OK" if ok else ("VIRTUAL_OK" if virtual_ok(d, lv, sdcc) else "WORSE")
        rows.append((prog, lv, cpu, ob, oc, bb, bc, verdict))
    return rows, refused


def virtual_ok(d, lv, sdcc):
    """The cell is virtual (no reference attains both per-axis bars), ours is Pareto-optimal among the
    references, and no worse than llvm-mos on both axes (fputable.verdict's rule, #8509; x86-specific, so not imported)."""
    ob, oc = d["ours"]
    ext = [d[c] for c in REQUIRED + sdcc]
    bb, bc = min(v[0] for v in ext), min(v[1] for v in ext)
    if any(v == (bb, bc) for v in ext):
        return False
    if any(v[0] <= ob and v[1] <= oc and (v[0] < ob or v[1] < oc) for v in ext):
        return False
    pb, pc = d["llvm-mos"]
    return ob <= pb and oc <= pc


def virtual_notes(cells, graded):
    """Lines for OUT.virtual.tsv: each VIRTUAL_OK cell with ours and every reference's (bytes, clocks)."""
    lines = ["# VIRTUAL_OK cells: no single reference attains both per-axis bars; ours is Pareto-optimal and no worse than llvm-mos on both axes (the pin-equivalent, an assumption: the pin has no MOS target)\n",
             "prog\tlevel\tcpu\tours\tbest_B\tbest_clk\tcolumns (bytes/clocks)\n"]
    for prog, lv, cpu, ob, oc, bb, bc, verdict in graded:
        if verdict == "VIRTUAL_OK":
            cols = " ".join(f"{c}={b}/{k:g}" for c, (b, k) in sorted(cells[(prog, lv, cpu)].items()) if c != "ours")
            lines.append(f"{prog}\t{lv}\t{cpu}\t{ob}/{oc:g}\t{bb}\t{bc:g}\t{cols}\n")
    return lines


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
    open(out + ".virtual.tsv", "w").write("".join(virtual_notes(cells, graded)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
