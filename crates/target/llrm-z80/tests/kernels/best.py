#!/usr/bin/env python3
"""The corpus table with, per row and metric, the best of several optimization levels, as the SDCC column is the best of
its variants.

    best.py --z80c PATH --ir-dir DIR --baseline sdcc.md [--levels 2,z]

Each level is measured by measure.py; a row counts only if every level's run was right (a refused or wrong row is
shown as such).
"""
import argparse, os, re, subprocess, sys

ap = argparse.ArgumentParser()
ap.add_argument("--z80c", required=True)
ap.add_argument("--ir-dir", required=True)
ap.add_argument("--baseline", required=True)
ap.add_argument("--levels", default="2,z")
ap.add_argument("--kernels")
args = ap.parse_args()
measure = os.path.join(os.path.dirname(os.path.abspath(__file__)), "measure.py")
runs = []
for level in args.levels.split(","):
    cmd = [sys.executable, "-I", measure, "--z80c", args.z80c, "--ir-dir", args.ir_dir, "--baseline", args.baseline, "-O", level]
    if args.kernels:
        cmd += ["--kernels", args.kernels]
    out = subprocess.run(cmd, capture_output=True, text=True)
    assert out.returncode == 0, out.stderr[-500:]
    rows = {}
    for line in out.stdout.splitlines():
        cells = [c.strip() for c in line.strip("|").split("|")]
        if len(cells) == 7 and cells[0] not in ("kernel", "---"):
            rows[cells[0]] = cells
    runs.append(rows)
verdict = lambda ours, theirs: "win" if ours < theirs else "tie" if ours == theirs else "LOSE"
print(f"| kernel | ours B (level) | ours T (level) | SDCC B | SDCC T | B | T |\n|---|---|---|---|---|---|---|")
for name in runs[0]:
    cells = [run[name] for run in runs]
    if any(not c[1].isdigit() for c in cells):
        print("| " + " | ".join(next(c for c in cells if not c[1].isdigit())) + " |")
        continue
    levels = args.levels.split(",")
    b = min(range(len(cells)), key=lambda i: int(cells[i][1]))
    t = min(range(len(cells)), key=lambda i: int(cells[i][2]))
    sb, st = int(cells[0][3]), int(cells[0][4])
    ob, ot = int(cells[b][1]), int(cells[t][2])
    print(f"| {name} | {ob} (-O{levels[b]}) | {ot} (-O{levels[t]}) | {sb} | {st} | {verdict(ob, sb)} | {verdict(ot, st)} |")
