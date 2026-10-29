#!/usr/bin/env python3
"""Cycles, instructions, branch and cache misses (user space) of one compile on both sides.

    cycles.py QCPORT [--llrm llrm-cc] [--llvm DIR] FILE.c ...   (a path under QCPORT/render is d_faces; others are plain)

Minimum cycles of --runs, the two compilers alternating. Prints one row per file and side, then the ratio llrm / LLVM.
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

EVENTS = ["instructions:u", "cycles:u", "branch-misses:u", "L1-dcache-load-misses:u"]
KEYWORDS = [f"-D{k}=" for k in "far huge near pascal cdecl __far _far __near _near".split()]


def stat(cmd: list[str]) -> dict[str, int]:
    done = subprocess.run(["perf", "stat", "-x,", *sum((["-e", e] for e in EVENTS), []), *cmd], capture_output=True, text=True)
    if done.returncode:
        sys.exit(f"failed: {' '.join(cmd[:3])}\n{done.stderr[-400:]}")
    out = {}
    for line in done.stderr.splitlines():
        f = line.split(",")
        if len(f) > 2 and f[0].isdigit():
            out[f[2].split(":")[0]] = int(f[0])
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("qcport")
    ap.add_argument("files", nargs="+")
    ap.add_argument("--llrm", default="llrm-cc")
    ap.add_argument("--llvm", default=str(Path.home() / "scratch/llvm-build/bin"))
    ap.add_argument("--runs", type=int, default=3)
    a = ap.parse_args()
    qc = Path(a.qcport).expanduser()
    inc = [x for d in ("", "render", "game", "host", "qgl") for x in ("-I", str(qc / d))]
    flags = ["-m32", "-march=i486", "-O2", "-fno-inline-functions"]
    obj = os.path.join(tempfile.mkdtemp(prefix="cycles-"), "o.o")
    print("| input | side | M instr | M cycles | IPC | M branch-miss | M L1d-miss |\n|---|---|---:|---:|---:|---:|---:|")
    for src in a.files:
        extra = KEYWORDS + inc
        cmds = {"LLVM": [f"{a.llvm}/clang", "-w", *flags, *extra, "-c", "-o", obj, src],
                "llrm": [a.llrm, "-c", "--target=x86_32-wat", "-march=i486", "-O2", "-fno-inline-functions", *extra, "-o", obj, src]}
        best: dict[str, dict[str, int]] = {}
        for _ in range(a.runs):
            for side, cmd in cmds.items():
                s = stat(cmd)
                if side not in best or s["cycles"] < best[side]["cycles"]:
                    best[side] = s
        for side, s in best.items():
            m = {k: v / 1e6 for k, v in s.items()}
            print(f"| {Path(src).name} | {side} | {m['instructions']:.0f} | {m['cycles']:.0f} | {s['instructions'] / s['cycles']:.2f} | "
                  f"{m['branch-misses']:.2f} | {m['L1-dcache-load-misses']:.1f} |")
        l, r = best["LLVM"], best["llrm"]
        print(f"| {Path(src).name} | llrm/LLVM | {r['instructions'] / l['instructions']:.1f}x | {r['cycles'] / l['cycles']:.1f}x | | "
              f"{r['branch-misses'] / l['branch-misses']:.1f}x | {r['L1-dcache-load-misses'] / l['L1-dcache-load-misses']:.1f}x |")


if __name__ == "__main__":
    main()
