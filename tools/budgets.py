#!/usr/bin/env python3
"""Compile-time budgets as counts (docs/architecture/compile-time-spec.md, section 6), in report mode.

    uv run --project tools python tools/budgets.py [FILE.c ...] [--axes] [--level -O1] [--json OUT]

Compiles each file (and, with --axes, the scaling axes at two sizes) with `LLRM_DEBUG=counts` and prints the counts the compiler
keeps (`llrm_mir::counters`) beside what each budget allows. Nothing fails: this is the baseline each stage is held to. The
counts are of work, not time, so they are the same on a loaded host.
"""
from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
sys.path.insert(0, str(ROOT / "crates/target/llrm-x86-m32/vsgcc"))
import llrmbin  # noqa: E402

# Budget per unit (spec section 10.4 and 6): the value each derived count must not pass, where one is set.
BUDGETS = {
    "analysis_per_pass_run": 0.5,
    "allocations_per_pipeline": 1.0,
}
SIZES = {"functions": (64, 128), "straight": (512, 1024), "branches": (256, 512), "live": (128, 256), "callers": (64, 128), "chain": (512, 1024), "nest": (16, 32), "cells": (256, 512), "joins": (128, 256), "fjoins": (64, 128), "cases": (256, 512), "unrolled": (16, 32)}


def parse(stderr: str) -> dict[str, int]:
    """The `[counts]` line of a compile's stderr, `name=value` each."""
    for line in reversed(stderr.splitlines()):
        if line.startswith("[counts] "):
            return {name: int(value) for name, _, value in (word.partition("=") for word in line[len("[counts] "):].split())}
    raise ValueError("no [counts] line: is LLRM_DEBUG=counts reaching the compiler?")


def derived(counts: dict[str, int]) -> dict[str, float]:
    """The ratios the budgets are stated in."""
    runs = max(1, counts["pass_runs"])
    analyses = counts["analysis_computed"] + counts["analysis_updated"]
    return {
        "analysis_per_pass_run": analyses / runs,
        "instructions_per_pass_run": counts["pass_run_instructions"] / runs,
        "allocations_per_pipeline": counts["allocations"] / max(1, counts["lir_pipeline_runs"]),
        "spills_and_splits": float(counts["spills"] + counts["splits"]),
    }


def compile_counts(source: Path, level: str, flags: list[str]) -> dict[str, int]:
    llrm = llrmbin.bin_dir() / "llrm-c"
    done = subprocess.run([str(llrm), "-m32", level, *flags, "-S", "-o", "/dev/null", str(source)], capture_output=True, text=True, env={"LLRM_DEBUG": "counts", "PATH": "/usr/bin:/bin"})
    if done.returncode:
        raise SystemExit(f"{source}: {done.stderr[-300:]}")
    return parse(done.stderr)


def slope(low: float, high: float) -> float:
    return math.log2(high / low) if low > 0 and high > 0 else float("nan")


def report(name: str, counts: dict[str, int]) -> dict:
    ratios = derived(counts)
    over = [f"{key} {ratios[key]:.2f} > {limit}" for key, limit in BUDGETS.items() if ratios[key] > limit]
    print(f"{name:28} runs {counts['pass_runs']:6}  analyses/run {ratios['analysis_per_pass_run']:5.2f}  insts/run {ratios['instructions_per_pass_run']:8.0f}  "
          f"alloc/pipeline {ratios['allocations_per_pipeline']:4.2f}  spills {counts['spills']:5} splits {counts['splits']:5}  forks {counts['function_forks']:4}"
          + ("   OVER: " + "; ".join(over) if over else ""))
    return {"counts": counts, "derived": ratios, "over": over}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("files", nargs="*", type=Path)
    parser.add_argument("--axes", action="store_true", help="the scaling axes at two sizes, with the slope of each count")
    parser.add_argument("--level", default="-O1")
    parser.add_argument("--json", type=Path)
    parser.add_argument("-I", dest="includes", action="append", default=[])
    args = parser.parse_args()
    flags = [flag for one in args.includes for flag in ("-I", one)]
    out: dict[str, dict] = {}
    for path in args.files:
        out[path.name] = report(path.name, compile_counts(path, args.level, flags))
    if args.axes:
        import scaling

        with tempfile.TemporaryDirectory() as tmp:
            for axis, sizes in SIZES.items():
                points = []
                for n in sizes:
                    source = Path(tmp) / f"{axis}{n}.c"
                    source.write_text(scaling.AXES[axis](n))
                    points.append(report(f"{axis} N={n}", compile_counts(source, args.level, [])))
                    out[f"{axis}-{n}"] = points[-1]
                work = [slope(p["counts"]["pass_run_instructions"], q["counts"]["pass_run_instructions"]) for p, q in [(points[0], points[1])]]
                analyses = [slope(p["counts"]["analysis_computed"] + p["counts"]["analysis_updated"], q["counts"]["analysis_computed"] + q["counts"]["analysis_updated"]) for p, q in [(points[0], points[1])]]
                print(f"{'':28} slope of pass_run_instructions {work[0]:.2f}, analyses {analyses[0]:.2f}")
    if args.json:
        args.json.write_text(json.dumps(out, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
