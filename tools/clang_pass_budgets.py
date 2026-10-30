#!/usr/bin/env python3
"""Clang's compile cost by phase and by pass, per IR instruction per run: the targets of plan step 4.2.

    QCPORT=<qb-quake src> tools/clang_pass_budgets.py [--levels O1,O2,Os] [source.c]

Runs clang's `-cc1` of the source (d_faces by default, with the bench flags) under callgrind and once with `-print-before-all`.
Phases are inclusive instructions of the new pass manager's module pipeline (the optimizer), of the legacy manager (code
generation: instruction selection to the object) and the rest (the front end). A pass's cost is the inclusive instructions of its
`run`; its runs and the IR instructions it ran over (the size of each IR dump before it, function, loop or module) come from the dump.
The columns are those of `tools/pass_budgets.py`, so the two tables line up.
"""
from __future__ import annotations

import argparse
import collections
import os
import re
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pin_clang  # noqa: E402

KEYWORDS = [f"-D{k}=" for k in "far huge near pascal cdecl __far _far __near _near".split()]
PASS_SYMBOL = re.compile(r"llvm::(\w+Pass)::run\(")
LEGACY_SYMBOL = re.compile(r"^llvm::(\w+)::(?:runOnFunction|runOnLoop|runOnMachineFunction)\(")
DUMP = re.compile(r"^; \*\*\* IR Dump Before (\w+) on .* \*\*\*$")
INSTRUCTION = re.compile(r"^  [^\s;]")


def cc1(clang: str, level: str, source: str, extra: list[str]) -> list[str]:
    """The `-cc1` command clang's driver would run."""
    out = subprocess.run(
        [clang, "-###", "-w", "-m32", "-march=i486", f"-{level}", "-fno-inline-functions", *KEYWORDS, *extra, "-c", source, "-o", "/dev/null"],
        capture_output=True, text=True,
    ).stderr
    line = next(one for one in out.splitlines() if '"-cc1"' in one)
    return shlex.split(line)


def dumped(text: str) -> dict[str, tuple[int, int]]:
    """Per pass, (runs, IR instructions it ran over) from `-print-before-all`."""
    found: dict[str, list[int]] = collections.defaultdict(lambda: [0, 0])
    current = None
    for line in text.splitlines():
        if one := DUMP.match(line):
            current = one[1]
            found[current][0] += 1
        elif current and INSTRUCTION.match(line):
            found[current][1] += 1
    return {name: (runs, size) for name, (runs, size) in found.items()}


def costs(report: str) -> tuple[dict[str, float], dict[str, float], dict[str, float]]:
    """(inclusive Minstr per pass `run`, of the phases, of the legacy passes by class) from callgrind_annotate --inclusive=yes."""
    legacy: dict[str, float] = {}
    passes: dict[str, float] = {}
    phases: dict[str, float] = {}
    total = 0.0
    for line in report.splitlines():
        if total_row := re.match(r"\s*([\d,]+) \(100\.0%\)\s+PROGRAM TOTALS", line):
            total = int(total_row[1].replace(",", "")) / 1e6
            continue
        row = re.match(r"\s*([\d,]+) \(\s*[\d.]+%\)\s+\S*?:(.*)$", line)
        if not row:
            continue
        cost, symbol = int(row[1].replace(",", "")) / 1e6, row[2]
        if (one := PASS_SYMBOL.search(symbol)) and symbol.startswith("llvm::") and one[1] not in passes:
            passes[one[1]] = cost
        if (old := LEGACY_SYMBOL.match(symbol)) and "Manager" not in old[1] and old[1] not in legacy:
            legacy[old[1]] = cost
        if symbol.startswith("llvm::PassManager<llvm::Module, llvm::AnalysisManager<llvm::Module>>::run(") and "'" not in symbol:
            phases.setdefault("optimizer", cost)
        if symbol.startswith("llvm::legacy::PassManager::run("):
            phases.setdefault("codegen", cost)
    phases["total"] = total
    phases["front end"] = total - phases.get("optimizer", 0.0) - phases.get("codegen", 0.0)
    return passes, phases, legacy


def table(passes: dict[str, float], sizes: dict[str, tuple[int, int]], budget: float) -> str:
    rows = [(name, sizes[name][0], sizes[name][1], cost) for name, cost in passes.items() if name in sizes and sizes[name][1]]
    out = [f"{'pass':28} {'runs':>5} {'IR instrs':>10} {'total M':>8} {'per IR instr':>12} {'x budget':>9}"]
    for name, runs, size, cost in sorted(rows, key=lambda one: -one[3]):
        per = cost * 1e6 / size
        out.append(f"{name:28} {runs:5} {size:10} {cost:8.1f} {per:12.0f} {per / budget:9.1f}")
    return "\n".join(out)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("source", nargs="?")
    parser.add_argument("--levels", default="O1,O2,Os")
    parser.add_argument("--clang", default=None, help="the clang to measure; by default the pin's, via tools/pin_clang.py (LLRM_PIN_CLANG)")
    parser.add_argument("--budget", type=float, default=220.0, help="LLVM's instructions per IR instruction per run (plan 4.1)")
    args = parser.parse_args()
    args.clang = args.clang or pin_clang.path()
    if args.source:
        source, extra = args.source, []
    else:
        qc = Path(os.environ["QCPORT"]).expanduser()
        source = str(qc / "render/d_faces.c")
        extra = [flag for d in ("", "render", "game", "host", "qgl") for flag in ("-I", str(qc / d))]
    for level in args.levels.split(","):
        command = cc1(args.clang, level, source, extra)
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "cg.out"
            done = subprocess.run(["valgrind", "--tool=callgrind", f"--callgrind-out-file={out}", *command, "-o", str(Path(tmp) / "o.o")], capture_output=True, text=True)
            if done.returncode:
                sys.exit(f"callgrind failed: {done.stderr[-400:]}")
            report = subprocess.run(["callgrind_annotate", "--inclusive=yes", "--threshold=100", str(out)], capture_output=True, text=True).stdout
        dump = subprocess.run([*command, "-o", "/dev/null", "-mllvm", "-print-before-all"], capture_output=True, text=True).stderr
        passes, phases, legacy = costs(report)
        print(f"-{level}: " + ", ".join(f"{name} {cost:.0f} M" for name, cost in phases.items()))
        print(table(passes, dumped(dump), args.budget))
        print("legacy passes (absolute M): " + ", ".join(f"{name} {cost:.1f}" for name, cost in sorted(legacy.items(), key=lambda one: -one[1])[:14]) + "\n")


if __name__ == "__main__":
    main()
