#!/usr/bin/env python3
"""Where llrm-c's compile instructions go, summed over the files compile-cost.py measures (the 66 programs and, when QCPORT and
QCPORT_INC are set, QCport's modules).

    tools/pass-profile.py [--level -O2] [--top 22] [llrm-c]

Each file is compiled with `LLRM_DEBUG=time`; its `[instr]` rows (a step's own user-space instructions, children excluded) are
summed per step, per group. The table is the question "what is the biggest step overall", which a profile of one file cannot
answer (d_faces alone is 10% of QCport and nothing like the rest). Steps that dominate today:
module analyses (summaries, call-effects, through-memory) and `mir gvn`; the allocator's spill path is a few percent.
Note: #1005 (interning) moves `analysis summaries` and `analysis call-effects`; rerun after it lands. Exit 77 without a counter.
"""
from __future__ import annotations

import argparse
import collections
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import llrmbin  # noqa: E402

_spec = importlib.util.spec_from_file_location("compile_cost", HERE / "compile-cost.py")
compile_cost = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(compile_cost)

ROW = re.compile(r"^\[instr\]\s+([\d.]+) (Minstr|Mcpu-ns) own\s+[\d.]+ \S+ total\s+\d+x (.+)$", re.M)


def own_work(compiler: Path, flags: list[str], source: Path, level: str) -> dict[str, float]:
    """One file's steps, by own Minstr; raises NoCounter where the rows are CPU time."""
    done = subprocess.run([str(compiler), level, *flags, str(source), "-o", os.devnull], capture_output=True, text=True, env={**os.environ, "LLRM_DEBUG": "time", "LLRM_TIME_TOP": "100000"})
    if done.returncode:
        raise SystemExit(f"{source.name}: exit {done.returncode}\n{done.stderr[-400:]}")
    rows = ROW.findall(done.stderr)
    if not rows or any(unit != "Minstr" for _, unit, _ in rows):
        raise compile_cost.NoCounter("llrm-c printed no instruction counts per step")
    return {name: float(own) for own, _, name in rows}


def summed(per_file: dict[str, dict[str, float]]) -> dict[str, collections.Counter]:
    """Steps by own Minstr, for QCport's modules and for the programs."""
    groups = {"QCport": collections.Counter(), "programs": collections.Counter()}
    for name, steps in per_file.items():
        groups["QCport" if name.startswith("qcport/") else "programs"].update(steps)
    return {group: total for group, total in groups.items() if total}


def table(group: str, total: collections.Counter, top: int) -> list[str]:
    whole = sum(total.values())
    return [f"{group}: {whole:.0f} Minstr"] + [f"  {100 * value / whole:5.1f}%  {name:<32} {value:9.0f}" for name, value in total.most_common(top)]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--level", default="-O2")
    parser.add_argument("--top", type=int, default=22)
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "4")))
    parser.add_argument("compiler", nargs="?", type=Path)
    args = parser.parse_args()
    compiler = args.compiler or llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory(dir=os.environ.get("CARGO_TARGET_DIR")) as tmp:
        files = compile_cost.files(Path(tmp))
        try:
            with ThreadPoolExecutor(args.jobs) as pool:
                got = dict(zip(files, pool.map(lambda name: own_work(compiler, files[name][1], files[name][0], args.level), files)))
        except compile_cost.NoCounter as why:
            print(f"SKIPPED: instruction counter unavailable ({why})")
            return 77
    for group, total in summed(got).items():
        print("\n".join(table(group, total, args.top)), "\n")
    print("#1005 (interning, not merged) will move `analysis summaries` and `analysis call-effects`.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
