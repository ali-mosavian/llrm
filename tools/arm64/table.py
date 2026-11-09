"""Judge table of arm64 bench cells against the best default gcc/clang column.

Input rows (tab separated): prog level compiler bytes instrs metric value flags (`ok` or `exception`: the program's value is not its .out, see expect.py;
`flags`: the canonical codegen flag set the column was built with, flagset.py). The `ours` column and every bar column of a cell must carry the same flags:
a table of columns built differently is refused, never judged. `--metric` names what the instruction
column must be (`kernel`: count.py --kernel, `whole`: count.py --whole); a row of another metric is refused, so a
header can never claim a metric its numbers were not counted with. O2/O3 verdict: net = instrs + 0.3 x bytes;
Os: bytes only. Verdicts are better / same / worse than the best of clang and gcc.
A row whose value is `unmeasured:<reason>` (a build or run that failed) is a hard line of the table, never a
silent gap and never rerun behind the table's back: the cell is listed, counted in the header and left out of the verdicts.
"""
import argparse
import collections
import sys

DEFAULT_BARS = ("clang", "gcc")


def verdicts(rows, ours="ours", bars=DEFAULT_BARS):
    cells = collections.defaultdict(dict)
    for prog, level, comp, nbytes, instrs in rows:
        cells[(prog, level)][comp] = (float(nbytes), float(instrs))
    out = {}
    for (prog, level), cols in sorted(cells.items()):
        if ours not in cols or not all(bar in cols for bar in bars):
            continue
        score = (lambda c: c[1] + 0.3 * c[0]) if level != "Os" else (lambda c: c[0])
        mine, best = score(cols[ours]), min(score(cols[bar]) for bar in bars)
        out[(prog, level)] = "better" if mine < best else "same" if mine == best else "worse"
    return out


def flag_mismatches(flag_sets, ours="ours", bars=DEFAULT_BARS):
    """One line per cell whose ours and bar columns were not built with the same codegen flags; `unknown` (no record) counts as a mismatch."""
    lines = []
    for (prog, level), cols in sorted(flag_sets.items()):
        if ours not in cols:
            continue
        for bar in bars:
            if bar not in cols:
                continue
            mine, theirs = cols[ours], cols[bar]
            if mine == theirs and mine != "unknown":
                continue
            diff = sorted(set(mine.split()) ^ set(theirs.split())) if "unknown" not in (mine, theirs) else ["no recorded flag set"]
            lines.append(f"{prog} {level}: {ours} vs {bar}: {' '.join(diff)}")
    return lines


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("rows")
    ap.add_argument("--metric", required=True, choices=("kernel", "whole"))
    ap.add_argument("--count-commit", required=True, help="the commit of count.py that counted the rows")
    args = ap.parse_args(argv)
    rows, exceptions, total, unmeasured, flag_sets = [], 0, 0, [], {}
    for number, line in enumerate(open(args.rows), 1):
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.rstrip("\n").split("\t")
        if len(fields) != 8:
            sys.exit(f"{args.rows}:{number}: {len(fields)} fields, a row has 8 (the last is the flag set, see flagset.py)")
        prog, level, comp, nbytes, instrs, metric, value, flags = fields
        total += 1
        if metric != args.metric:
            sys.exit(f"{args.rows}:{number}: row counted as {metric!r}, header says {args.metric!r}")
        if value.startswith("unmeasured"):
            unmeasured.append((prog, level, comp, value))
            continue
        exceptions += value == "exception"
        rows.append((prog, level, comp, nbytes, instrs))
        flag_sets.setdefault((prog, level), {})[comp] = flags
    mismatches = flag_mismatches(flag_sets)
    if mismatches:
        sys.exit("flag sets differ between columns of a cell (not a comparison):\n" + "\n".join(mismatches))
    result = verdicts(rows)
    # A cell with an unmeasured column has no verdict.
    gone = {(prog, level) for prog, level, _, _ in unmeasured}
    result = {cell: verdict for cell, verdict in result.items() if cell not in gone}
    rule = "range [first function, report) minus main" if args.metric == "kernel" else "range: the whole process"
    print(f"# instrs metric: {args.metric} (count.py --{args.metric}; {rule}); count.py commit {args.count_commit}")
    measured = total - len(unmeasured)
    print(f"# value check: {measured - exceptions} of {measured} measured cells value == .out; {exceptions} of {measured}: value != .out on all columns (named exceptions, expect.py)")
    print(f"# UNMEASURED: {len(unmeasured)} of {total} cells" + ("" if not unmeasured else " (listed below; no verdict for their programs at that level)"))
    print(f"# bar: best of {'/'.join(DEFAULT_BARS)}; O2/O3 net = instrs + 0.3 B, Os bytes")
    print("prog\tlevel\tverdict")
    for (prog, level), verdict in result.items():
        print(f"{prog}\t{level}\t{verdict}")
    for prog, level, comp, value in unmeasured:
        print(f"{prog}\t{level}\tUNMEASURED\t{comp}\t{value}")
    for level in sorted({lv for _, lv in result}):
        tally = collections.Counter(v for (_, lv), v in result.items() if lv == level)
        print(f"# {level}: " + ", ".join(f"{k} {tally[k]}" for k in ("better", "same", "worse")))


if __name__ == "__main__":
    main()
