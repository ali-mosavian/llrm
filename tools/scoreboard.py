#!/usr/bin/env python3
"""llrm against gcc: instructions:u per level, and where llrm's go, by family.

    tools/scoreboard.py [--levels O1 O2] [--jobs N] [--json FILE] [llrm-c]

Two sets, from tools/compile-cost.py's files. The 66 programs (wrapped as vsgcc compiles them) are what gcc can compile: llrm/gcc
over the whole compile, per level, and gcc's `-fsyntax-only` as its parse. QCport's modules are Open Watcom DOS C, which gcc
cannot (57 of 65 fail on its headers and keywords): llrm alone, so that the family table is of real code. A file that gcc fails on
is left out of the ratio and counted. llrm's work per step is `LLRM_DEBUG=time`'s [instr] rows (children, the C parser one, are in
the perf count), grouped by family. Needs QCPORT and QCPORT_INC for the second set, gcc and perf.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import math
import os
import re
import subprocess
import sys
import tempfile
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import llrmbin  # noqa: E402
spec = importlib.util.spec_from_file_location("compile_cost", HERE / "compile-cost.py")
compile_cost = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compile_cost)

SCRATCH = os.environ.get("CARGO_TARGET_DIR")  # never /tmp
ROW = re.compile(r"^\[instr\]\s+([\d.]+) Minstr own\s+[\d.]+ Minstr total\s+\d+x (.+)$", re.M)

FAMILIES = [
    ("IPA / alias / summaries", lambda k: k in ("mir interprocedural", "mir stamp") or k.startswith("summaries")
     or k in ("analysis call-effects", "analysis globals-aa", "analysis pointer-values", "analysis points-to",
              "analysis through-memory", "analysis annotated", "analysis accesses", "analysis exposed-frames",
              "analysis callbacks")),
    ("scalar mid-end passes", lambda k: k.startswith("mir ") and k not in ("mir interprocedural", "mir stamp", "mir pipeline")),
    ("other mir analyses", lambda k: k.startswith("analysis ")),
    ("register allocation", lambda k: k.split()[0] in ("regalloc", "split", "spill", "intervals", "facts", "after",
                                                        "siblings", "candidate", "ssa", "classes")),
    ("lir passes", lambda k: k.startswith("lir ")),
    ("isel / assemble / emit", lambda k: k in ("isel", "assemble") or k.startswith(("masm", "frame"))),
    ("frontend rows", lambda k: k.startswith(("frontend", "hir"))),
]


GCC = ["gcc", "-m32", "-march=i486", "-w", "-Dfar=", "-fno-pic", "-fno-stack-protector", "-fcf-protection=none",
       "-fno-asynchronous-unwind-tables"]


def instructions(command: list[str], env: dict | None = None, ok: bool = False) -> tuple[int | None, str]:
    with tempfile.NamedTemporaryFile("r", dir=SCRATCH) as out, llrmbin.with_object(command) as real:
        done = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", "-o", out.name, "--", *real],
                              capture_output=True, text=True, env={**os.environ, **(env or {})})
        counts = [line.split(",") for line in out.read().splitlines() if "instructions" in line]
    if ok and done.returncode:
        return None, done.stderr
    if done.returncode or not counts or not counts[0][0].isdigit():
        raise SystemExit(f"{' '.join(command[-3:])}: exit {done.returncode}\n{done.stderr[-300:]}")
    return int(counts[0][0]), done.stderr


def measure(llrm: Path, levels: list[str], jobs: int) -> list[dict]:
    with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
        files = compile_cost.files(Path(tmp))

        def one(job):
            name, level, source, flags = job
            made, err = instructions([str(llrm), level, *flags, str(source), "-o", llrmbin.OBJECT],
                                     {"LLRM_DEBUG": "time", "LLRM_TIME_TOP": "100000"}, ok=True)
            rows = {m.group(2): float(m.group(1)) for m in ROW.finditer(err)}
            run = {"file": name, "set": "qcport" if name.startswith("qcport/") else "programs", "level": level, "llrm": made,
                   "rows": rows, "gcc": None, "gcc_parse": None}
            if run["set"] == "programs":
                run["gcc"], _ = instructions([*GCC, level, "-c", str(source), "-o", os.devnull], ok=True)
                run["gcc_parse"], _ = instructions([*GCC, "-fsyntax-only", str(source)], ok=True)
            return run

        todo = [(n, l, s, f) for n, (s, f) in files.items() for l in levels]
        with ThreadPoolExecutor(jobs) as pool:
            return list(pool.map(one, todo))


def refused(runs: list[dict]) -> list[str]:
    """The rows a compile failed in: a count of instructions of a compiler that stopped at its first error is not a
    measurement (gcc on QCport read 'parse is 90% of the compile' from 57 failed runs), so such a row is in no total."""
    return [f"{r['file']} {r['level']}: {who} failed" for r in runs for who, key in (("llrm", "llrm"), ("gcc", "gcc"))
            if r[key] is None and (key == "llrm" or r["set"] == "programs")]


def clean(runs: list[dict]) -> list[dict]:
    """The runs every compiler asked of made."""
    return [r for r in runs if r["llrm"] is not None and (r["set"] != "programs" or (r["gcc"] and r["gcc_parse"]))]


def families(rr: list[dict]) -> list[str]:
    own = Counter()
    for r in rr:
        own.update(r["rows"])
    total = sum(own.values())
    out = []
    left = dict(own)
    for name, belongs in FAMILIES:
        share = sum(left.pop(k) for k in [k for k in left if belongs(k)])
        out.append(f"  {100 * share / total:5.1f}%  {share:9,.1f}  {name}")
    out.append(f"  {100 * sum(left.values()) / total:5.1f}%  {sum(left.values()):9,.1f}  other ({', '.join(sorted(left, key=left.get, reverse=True)[:3])})")
    out.append("  top rows: " + ", ".join(f"{k} {100 * v / total:.1f}%" for k, v in own.most_common(8)))
    return out


def report(all_runs: list[dict]) -> str:
    out = []
    runs = clean(all_runs)
    lost = refused(all_runs)
    if lost:
        out.append(f"REFUSED {len(lost)} rows, left out of every total:")
        out += [f"  {line}" for line in lost]
    for level in sorted({r["level"] for r in runs}):
        both = [r for r in runs if r["level"] == level and r["set"] == "programs"]
        every = [r for r in runs if r["level"] == level and r["set"] == "programs"]
        if both:
            llrm, gcc, parse = (sum(r[k] for r in both) / 1e6 for k in ("llrm", "gcc", "gcc_parse"))
            spread = sorted(r["llrm"] / r["gcc"] for r in both)
            geo = math.exp(sum(math.log(r["llrm"] / r["gcc"]) for r in both) / len(both))
            out.append(f"{level} programs: {len(both)} of {len(every)} files both compile; llrm {llrm:,.1f} Minstr, gcc {gcc:,.1f} "
                       f"(parse {parse:,.1f}, {100 * parse / gcc:.0f}%); llrm/gcc {llrm / gcc:.2f}x total, {geo:.2f}x geomean, "
                       f"per file min {spread[0]:.2f} median {spread[len(spread) // 2]:.2f} max {spread[-1]:.2f}")
            out += families(every)
        qc = [r for r in runs if r["level"] == level and r["set"] == "qcport"]
        if qc:
            out.append(f"{level} qcport: {len(qc)} files, llrm {sum(r['llrm'] for r in qc) / 1e6:,.0f} Minstr (gcc cannot compile them)")
            out += families(qc)
            heavy = sorted(qc, key=lambda r: -r["llrm"])[:5]
            out.append("  heaviest: " + ", ".join(f"{r['file']} {r['llrm'] / 1e6:,.0f}" for r in heavy))
    return "\n".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--levels", nargs="*", default=["O1", "O2"], help="without the dash: O1 O2 Os")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "4")))
    parser.add_argument("--json", type=Path, help="write the runs here")
    parser.add_argument("llrm", nargs="?", type=Path)
    args = parser.parse_args()

    runs = measure(args.llrm or llrmbin.bin_dir() / "llrm-c", [f"-{level.lstrip('-')}" for level in args.levels], args.jobs)
    if args.json:
        args.json.write_text(json.dumps(runs))
    print(report(runs))
    return 1 if refused(runs) else 0


if __name__ == "__main__":
    sys.exit(main())
