#!/usr/bin/env python3
"""Cells where a gcc column is lower than ours on bytes, clocks or instructions, ranked by the largest relative margin.

    gcc_leads.py MATRIX.tsv GCC.tsv [GCC2.tsv ...] [--cpu 6502|65c02] [--top N]

gcc = the lower of the given gcc tables per cell and metric (the bar). A cell is (prog, level); ours is the row of
the same level (ours / ours-65c02). n/a and flagged helper/table cells are left out, as in vs_6502.py.
"""
import argparse, collections, sys


def rows(path):
    for l in open(path):
        if l.startswith("#") or l.startswith("target"):
            continue
        f = l.rstrip("\n").split("\t") + [""] * 8
        yield f[:8]


def num(x):
    return int(x) if x.strip().isdigit() else None


def leads(matrix, gcc_tsvs, cpu="6502"):
    gname = "gcc" + ("-65c02" if cpu == "65c02" else "")
    oname = "ours" + ("-65c02" if cpu == "65c02" else "")
    g = {}
    for p in gcc_tsvs:
        for t, prog, lvl, c, b, k, i, note in rows(p):
            if c.split("-noivopts")[0] != gname or not b or note.startswith(("hand helper", "table")):
                continue
            cur = g.setdefault((prog, lvl), [None, None, None])
            for n, v in enumerate((num(b), num(k), num(i))):
                if v is not None and (cur[n] is None or v < cur[n]):
                    cur[n] = v
    o = {(prog, lvl): (num(b), num(k), num(i)) for t, prog, lvl, c, b, k, i, note in rows(matrix) if c == oname}
    out = []
    for cell, gv in g.items():
        ov = o.get(cell)
        if not ov:
            continue
        d = []
        for name, a, b in zip(("B", "clk", "insn"), gv, ov):
            if a is not None and b is not None and a < b:
                d.append((name, a, b, 1 - a / b))
        if d:
            out.append((max(x[3] for x in d), cell, d))
    return sorted(out, reverse=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="+")
    ap.add_argument("--cpu", default="6502")
    ap.add_argument("--top", type=int, default=0)
    a = ap.parse_args()
    res = leads(a.files[0], a.files[1:], a.cpu)
    for m, (prog, lvl), d in res[: a.top or None]:
        print(f"{prog}\t{lvl}\t" + "  ".join(f"{n} {x}<{y} (-{100 * r:.0f}%)" for n, x, y, r in d))
    print(f"# {len(res)} cells", file=sys.stderr)


if __name__ == "__main__":
    main()
