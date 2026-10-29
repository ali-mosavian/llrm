#!/usr/bin/env python3
"""Peak resident set (kB) of clang -S and llrm-cc against function size, and the slope per pre-opt IR instruction.

    rss.py [--llrm llrm-cc] [--llvm DIR] [--shape mem] [--sizes 100,400,1600]

Max RSS from wait4's rusage (`/usr/bin/time -f %M`), minimum of three; the empty-input run is taken off.
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from ladder_gen import generate  # noqa: E402
from ladder import FLAGS, LLRM_FLAGS, ir_count  # noqa: E402


def rss(cmd: list[str]) -> int:
    best = None
    for _ in range(3):
        err = subprocess.run(["/usr/bin/time", "-f", "%M", *cmd], capture_output=True, text=True).stderr
        v = int(err.strip().splitlines()[-1])
        best = v if best is None else min(best, v)
    return best


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--llrm", default="llrm-cc")
    ap.add_argument("--llvm", default=str(Path.home() / "scratch/llvm-build/bin"))
    ap.add_argument("--shape", default="mem")
    ap.add_argument("--sizes", default="100,400,1600")
    a = ap.parse_args()
    with tempfile.TemporaryDirectory() as d:
        base = {}
        for n in [1, *map(int, a.sizes.split(","))]:
            c, raw = f"{d}/f.c", f"{d}/raw.ll"
            Path(c).write_text(generate(a.shape, n))
            subprocess.run([f"{a.llvm}/clang", *FLAGS, "-O2", "-Xclang", "-disable-llvm-passes", "-emit-llvm", "-S", c, "-o", raw], check=True)
            ir = ir_count(Path(raw).read_text())
            llvm = rss([f"{a.llvm}/clang", *FLAGS, "-O2", "-S", "-o", os.devnull, c])
            llrm = rss([a.llrm, *LLRM_FLAGS, "-O2", "-S", "-o", os.devnull, c])
            if n == 1:
                base = {"llvm": llvm, "llrm": llrm}
                print(f"N=1: IR {ir}, clang {llvm} kB, llrm-cc {llrm} kB")
                continue
            print(f"N={n}: IR {ir}, clang +{llvm - base['llvm']} kB ({(llvm - base['llvm']) * 1024 / ir:.0f} B/IR), "
                  f"llrm-cc +{llrm - base['llrm']} kB ({(llrm - base['llrm']) * 1024 / ir:.0f} B/IR)")


if __name__ == "__main__":
    main()
