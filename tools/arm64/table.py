"""Judge table of arm64 bench cells against the best default gcc/clang column.

Input rows (tab separated): prog level compiler bytes instrs metric value (`ok` or `exception`: the program's value is not its .out, see expect.py). `--metric` names what the instruction
column must be (`kernel`: count.py --kernel, `whole`: count.py --whole); a row of another metric is refused, so a
header can never claim a metric its numbers were not counted with. O2/O3 verdict: net = instrs + 0.3 x bytes;
Os: bytes only. Verdicts are better / same / worse than the best of clang and gcc.
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


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("rows")
    ap.add_argument("--metric", required=True, choices=("kernel", "whole"))
    ap.add_argument("--count-commit", required=True, help="the commit of count.py that counted the rows")
    args = ap.parse_args(argv)
    rows, exceptions, total = [], 0, 0
    for number, line in enumerate(open(args.rows), 1):
        if not line.strip() or line.startswith("#"):
            continue
        prog, level, comp, nbytes, instrs, metric, value = line.rstrip("\n").split("\t")
        exceptions += value == "exception"
        total += 1
        if metric != args.metric:
            sys.exit(f"{args.rows}:{number}: row counted as {metric!r}, header says {args.metric!r}")
        rows.append((prog, level, comp, nbytes, instrs))
    result = verdicts(rows)
    rule = "range [first function, report) minus main" if args.metric == "kernel" else "range: the whole process"
    print(f"# instrs metric: {args.metric} (count.py --{args.metric}; {rule}); count.py commit {args.count_commit}")
    print(f"# value check: {total - exceptions} of {total} cells value == .out; {exceptions} of {total} cells: value != .out on all columns (named exceptions, expect.py)")
    print(f"# bar: best of {'/'.join(DEFAULT_BARS)}; O2/O3 net = instrs + 0.3 B, Os bytes")
    print("prog\tlevel\tverdict")
    for (prog, level), verdict in result.items():
        print(f"{prog}\t{level}\t{verdict}")
    for level in sorted({lv for _, lv in result}):
        tally = collections.Counter(v for (_, lv), v in result.items() if lv == level)
        print(f"# {level}: " + ", ".join(f"{k} {tally[k]}" for k in ("better", "same", "worse")))


if __name__ == "__main__":
    main()
