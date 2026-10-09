#!/usr/bin/env python3
"""How many times each body goes through the MIR pipeline and the backend's allocators, and how much of that work changes nothing.

    tools/pipeline-runs.py [--level -O2] [--group qcport|programs] [llrm-c]

Compiles each file of compile-cost.py's set with `LLRM_DEBUG=runs,time,regalloc` and prints, per body: pipeline runs (the first,
then what the interprocedural step reruns it for), fixed-point rounds, pass runs; the work of the pass runs that changed nothing
('idle', billed with whatever analyses they computed first) by round and by pass; and the backend's allocations per function.
Exit 77 without a counter.
"""
from __future__ import annotations

import argparse
import collections
import importlib.util
import os
import re
import statistics
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

RUN = re.compile(r'^\[runs\] body (-?\d+) trigger "([^"]*)": (\d+) fixed points, (\d+) rounds, (\d+) pass runs, (\d+) skipped as settled, (\d+) changes')
STEP = re.compile(r"^\[runs\] step (\S+) (idle|changed) (\d+)")
TOTAL = re.compile(r"^\[instr\] total ([\d.]+) Minstr")
SPAN = re.compile(r"^\[instr\]\s+([\d.]+) Minstr own\s+([\d.]+) Minstr total\s+(\d+)x (.+)$")
ALLOCATED = re.compile(r"^\[regalloc\] \S+: \d+ insns", re.M)


def parsed(text: str) -> dict:
    """One compile's `runs`, `time` and `regalloc` lines: bodies' runs by trigger kind, steps, the total, the backend's spans."""
    bodies: dict[str, dict[str, list[int]]] = collections.defaultdict(lambda: collections.defaultdict(lambda: [0, 0, 0]))
    steps, spans, total = [], {}, 0.0
    for line in text.splitlines():
        if m := RUN.match(line):
            body, trigger, _, rounds, passes = m.group(1), m.group(2), *m.group(3, 4, 5)
            row = bodies[body][re.sub(r"\d+", "", trigger) or "first"]
            row[0] += 1
            row[1] += int(rounds)
            row[2] += int(passes)
        elif m := STEP.match(line):
            steps.append((m.group(1), m.group(2), int(m.group(3))))
        elif m := TOTAL.match(line):
            total = float(m.group(1)) * 1e6
        elif m := SPAN.match(line):
            spans[m.group(4)] = (float(m.group(2)) * 1e6, int(m.group(3)))
    return {"bodies": bodies, "steps": steps, "total": total, "spans": spans, "allocated": len(ALLOCATED.findall(text))}


def report(compiles: list[dict]) -> str:
    out = []
    rows = [(sum(r[0] for r in kinds.values()), sum(r[1] for r in kinds.values()), sum(r[2] for r in kinds.values())) for c in compiles for kinds in c["bodies"].values()]
    for i, name in enumerate(("pipeline runs", "rounds", "pass runs")):
        values = sorted(row[i] for row in rows)
        out.append(f"{name} per body: median {statistics.median(values):g}, p90 {values[int(len(values) * .9)]}, worst {values[-1]} ({len(values)} bodies)")
    kinds, runs = collections.Counter(), collections.Counter()
    for c in compiles:
        for per in c["bodies"].values():
            for kind, row in per.items():
                kinds[kind] += row[2]
                runs[kind] += row[0]
    whole = sum(kinds.values())
    out.append("pass runs by trigger: " + ", ".join(f"{k} {v / whole:.1%} ({runs[k]} runs)" for k, v in kinds.most_common()))
    total = sum(c["total"] for c in compiles)
    idle, changed = collections.Counter(), collections.Counter()
    by_round, by_pass, all_pass = collections.Counter(), collections.Counter(), collections.Counter()
    for c in compiles:
        for stage, kind, spent in c["steps"]:
            m = re.match(r".*?r(\d+)-(.+)$", stage)
            round_, name = (int(m.group(1)), m.group(2)) if m else (0, stage)
            (idle if kind == "idle" else changed)[0] += spent
            all_pass[name] += spent
            if kind == "idle":
                by_round[min(round_, 5)] += spent
                by_pass[name] += spent
    passes = idle[0] + changed[0]
    out.append(f"compile {total / 1e9:.1f} G; MIR function passes {passes / 1e9:.1f} G ({passes / total:.0%}); those that changed nothing {idle[0] / 1e9:.1f} G ({idle[0] / passes:.0%} of passes, {idle[0] / total:.0%} of the compile)")
    out.append("idle by round (5: five and later): " + ", ".join(f"r{k} {v / 1e9:.1f} G" for k, v in sorted(by_round.items())))
    out.append("idle by pass: " + ", ".join(f"{k} {v / 1e9:.1f} G ({v / all_pass[k]:.0%} of its work)" for k, v in by_pass.most_common(8)))
    functions = sum(c["allocated"] for c in compiles)
    spans, calls = collections.Counter(), collections.Counter()
    for c in compiles:
        for k, (t, n) in c["spans"].items():
            spans[k] += t
            calls[k] += n
    out.append(f"backend, {functions} functions allocated:")
    for k in ("assemble", "candidate first frame", "candidate allocator alone", "candidate spiller", "lir regalloc", "regalloc base", "regalloc trial", "regalloc spill", "mir pipeline"):
        if k in calls:
            out.append(f"  {k:<28} {spans[k] / 1e9:7.1f} G {calls[k]:6d} calls = {calls[k] / max(functions, 1):.2f} per function")
    return "\n".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--level", default="-O2")
    parser.add_argument("--group", choices=("qcport", "programs"), default="programs")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "4")))
    parser.add_argument("compiler", nargs="?", type=Path)
    args = parser.parse_args()
    compiler = args.compiler or llrmbin.bin_dir() / "llrm-c"
    env = {**os.environ, "LLRM_DEBUG": "runs,time,regalloc", "LLRM_TIME_TOP": "100000"}
    with tempfile.TemporaryDirectory(dir=compile_cost.SCRATCH) as tmp:
        files = {k: v for k, v in compile_cost.files(Path(tmp)).items() if k.startswith("qcport/") == (args.group == "qcport")}

        def one(item):
            src, flags = item[1]
            return subprocess.run([str(compiler), args.level, *flags, str(src), "-o", os.devnull], env=env, capture_output=True, text=True).stderr

        with ThreadPoolExecutor(args.jobs) as pool:
            texts = list(pool.map(one, files.items()))
    compiles = [parsed(text) for text in texts]
    if not compiles or any(not c["total"] for c in compiles):
        print("SKIPPED: instruction counter unavailable")
        return 77
    print(f"{args.group} {args.level}\n" + report(compiles))
    return 0


if __name__ == "__main__":
    sys.exit(main())
