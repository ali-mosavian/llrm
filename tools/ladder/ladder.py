#!/usr/bin/env python3
"""Compile-cost ladder: instructions:u of LLVM's opt/llc and llrm-cc on identical generated C.

    ladder.py [--llvm DIR] [--llrm llrm-cc] [--shapes a,b] [--sizes 25,50,...] [--runs 3] OUT.json

Per (shape, N): IR instruction count before optimization, `opt -passes='default<O2>'`, `llc -O2`, clang end to end,
llrm-cc end to end, and llrm's optimizer/backend split from LLRM_DEBUG=time. Minimum of --runs, alternating sides.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from ladder_gen import SHAPES, generate  # noqa: E402

FLAGS = ["-w", "-m32", "-march=i486", "-fno-inline-functions"]
LLRM_FLAGS = ["--target=x86_32-wat", "-march=i486"]  # llrm-cc takes its target as a row


def instr(cmd: list[str], env: dict[str, str] | None = None) -> int:
    done = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", *cmd], capture_output=True, text=True,
                          env={**os.environ, **(env or {})})
    if done.returncode:
        sys.exit(f"failed: {' '.join(cmd[:3])}\n{done.stderr[-400:]}")
    for line in done.stderr.splitlines():
        f = line.split(",")
        if len(f) > 2 and f[2].startswith("instructions"):
            return int(f[0])
    sys.exit("no instructions line: " + done.stderr[-200:])


def ir_count(text: str) -> int:
    body = re.findall(r"^define .*?^}", text, re.S | re.M)
    return sum(1 for fn in body for l in fn.splitlines()[1:-1] if l.startswith("  ") and not l.startswith("  ;"))


def llrm_split(llrm: str, c: str) -> dict[str, float]:
    """Inclusive Minstr of `mir pipeline` (optimizer) and `assemble` (backend) from `LLRM_DEBUG=time`'s `[instr]` table."""
    err = subprocess.run([llrm, *LLRM_FLAGS, "-O2", "-S", "-o", os.devnull, c], capture_output=True, text=True,
                         env={**os.environ, "LLRM_DEBUG": "time", "LLRM_TIME_TOP": "700"}).stderr
    out: dict[str, float] = {}
    for line in err.split("[instr] by own work")[1].split("nesting")[0].splitlines():
        m = re.search(r"[\d.]+ Minstr own\s+([\d.]+) Minstr total\s+\d+x (mir pipeline|assemble)\s*$", line)
        if m:
            out[{"mir pipeline": "optimizer", "assemble": "backend"}[m[2]]] = float(m[1])
    if len(out) != 2:
        sys.exit("llrm phase lines missing: " + err[-300:])
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--llvm", default=str(Path.home() / "scratch/llvm-build/bin"))
    ap.add_argument("--llrm", default="llrm-cc")
    ap.add_argument("--shapes", default=",".join(SHAPES))
    ap.add_argument("--sizes", default="25,50,100,200,400,800,1600")
    ap.add_argument("--runs", type=int, default=3)
    a = ap.parse_args()
    clang, opt, llc = (f"{a.llvm}/{t}" for t in ("clang", "opt", "llc"))
    rows = []
    with tempfile.TemporaryDirectory() as d:
        for shape in a.shapes.split(","):
            for n in map(int, a.sizes.split(",")):
                c, raw, o2, null = f"{d}/f.c", f"{d}/raw.ll", f"{d}/o2.ll", os.devnull
                Path(c).write_text(generate(shape, n))
                subprocess.run([clang, *FLAGS, "-O2", "-Xclang", "-disable-llvm-passes", "-emit-llvm", "-S", c, "-o", raw], check=True)
                subprocess.run([opt, "-passes=default<O2>", "-S", raw, "-o", o2], check=True)
                cmds = {
                    "opt": [opt, "-passes=default<O2>", "-disable-output", raw],
                    "llc": [llc, "-O2", "-mtriple=i486-pc-linux-gnu", o2, "-o", null],
                    "clang": [clang, *FLAGS, "-O2", "-S", c, "-o", null],
                    "llrm": [a.llrm, *LLRM_FLAGS, "-O2", "-S", "-o", null, c],
                }
                best: dict[str, int] = {}
                for _ in range(a.runs):
                    for k, cmd in cmds.items():
                        v = instr(cmd)
                        best[k] = min(best.get(k, v), v)
                row = {"shape": shape, "n": n, "ir": ir_count(Path(raw).read_text()),
                       "ir_o2": ir_count(Path(o2).read_text()), **best, "llrm_phases": llrm_split(a.llrm, c)}
                rows.append(row)
                print(json.dumps({k: v for k, v in row.items() if k != "llrm_phases"}), flush=True)
    Path(a.out).write_text(json.dumps(rows, indent=1))


if __name__ == "__main__":
    main()
