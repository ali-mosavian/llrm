"""Judges an allocator change the same way each time: what the allocator still spills, the clocks of the 16-bit bench, the compile.

    alloc_judge.py spills BIN_DIR [--candidates allocator|spiller] [--level -O2]
    alloc_judge.py compile BIN_DIR [--candidates ...] [--level -O2]
    alloc_judge.py compare BASE.json NEW.json

`spills` compiles every bench C source for m16 and sums, per function, the values the first allocation spilled (the `regalloc` debug
line), so a change that lets the allocator place what it spilled shows as a fall. `compile` counts the instructions of compiling
QCport's d_faces. `compare` reads two `tools/bench/bench.py --no-judge --json` files: the geomean of each counter over the rows both
have, the rows worse by more than half a percent, and the worst.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
QCPORT = Path(os.environ.get("DFACES_SRC") or os.environ.get("QCPORT") or Path.home() / "scratch/qbq-llrm/src")
COUNTERS = ("clocks", "instructions", "memory_operands", "code_bytes")


def first_spills(stderr: str) -> dict[str, int]:
    """Per function, the values its first allocation spilled: a later line of the same function is a trial."""
    first: dict[str, int] = {}
    for line in stderr.splitlines():
        found = re.match(r"\[regalloc\] (\S+): \d+ insns, (\d+) spilled", line)
        if found and found.group(1) not in first:
            first[found.group(1)] = int(found.group(2))
    return first


def residual(bin_dir: Path, candidates: str | None, level: str) -> dict[str, dict[str, int]]:
    env = {**os.environ, "LLRM_DEBUG": "regalloc", "LLRM_VERIFY": "0"}
    if candidates:
        env["LLRM_CANDIDATES"] = candidates
    out: dict[str, dict[str, int]] = {}
    for source in sorted((ROOT / "bench").glob("*/*.c")):
        done = subprocess.run([str(bin_dir / "llrm-c"), "-m16", level, "-march=i486", str(source), "-S", "-o", os.devnull],
                              env=env, capture_output=True, text=True)
        out[f"{source.parent.name}"] = first_spills(done.stderr)
    return out


def ratios(base: dict, new: dict, counter: str) -> dict[str, float]:
    return {row: new[row][counter] / base[row][counter] for row in base
            if row in new and base[row].get(counter) and new[row].get(counter)}


def summary(base: dict, new: dict) -> list[str]:
    lines = []
    for counter in COUNTERS:
        found = ratios(base, new, counter)
        if not found:
            continue
        geomean = math.exp(sum(map(math.log, found.values())) / len(found))
        worse = sorted(((round(value, 3), row) for row, value in found.items() if value > 1.005), reverse=True)
        lines.append(f"{counter:16s} n={len(found)} geomean {geomean:.4f} worse {len(worse)} worst {worse[:3]}")
    return lines


def compile_instructions(bin_dir: Path, candidates: str | None, level: str) -> int:
    includes = [flag for d in ("", "render", "game", "host", "qgl") for flag in ("-I", str(QCPORT / d))]
    env = {**os.environ, **({"LLRM_CANDIDATES": candidates} if candidates else {})}
    done = subprocess.run(["perf", "stat", "-e", "instructions:u", "-x,", str(bin_dir / "llrm-c"), "-m16", "-march=i486", level, *includes,
                           "-S", "-o", os.devnull, str(QCPORT / "render/d_faces.c")], env=env, capture_output=True, text=True, check=True)
    return next(int(line.split(",")[0]) for line in done.stderr.splitlines() if "instructions" in line)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("what", choices=("spills", "compile", "compare"))
    parser.add_argument("paths", nargs="+")
    parser.add_argument("--candidates", choices=("allocator", "spiller"))
    parser.add_argument("--level", default="-O2")
    args = parser.parse_args()
    if args.what == "compare":
        base, new = (json.load(open(path)) for path in args.paths[:2])
        print("\n".join(summary(base, new)))
    elif args.what == "spills":
        found = residual(Path(args.paths[0]), args.candidates, args.level)
        total = sum(sum(counts.values()) for counts in found.values())
        print(f"spilled values, first allocations: {total} in {sum(1 for c in found.values() for n in c.values() if n)} functions")
    else:
        print(f"d_faces compile {compile_instructions(Path(args.paths[0]), args.candidates, args.level) / 1e6:.0f} Minstr")


if __name__ == "__main__":
    sys.exit(main())
