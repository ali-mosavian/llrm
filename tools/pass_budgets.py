#!/usr/bin/env python3
"""Instructions per IR instruction per pass run, the table plan step 4.1 judges the passes by.

    QCPORT=<qb-quake src> tools/pass_budgets.py [--llrm llrm-cc] [--levels O1,O2,Os] [--budget 300] [source.c]

One compile of d_faces (or `source.c`) per level with `LLRM_DEBUG=counts,time`. A pass's cost is its `mir NAME` row's total
instructions (the analyses it made the manager compute are inside it, as `counts` charges them); its size is the sum of the
instructions of the functions it ran over, from `[counts] pass NAME R runs I instructions`. Prints the passes by how far over the
budget they are: instructions per IR instruction per run against the budget (LLVM spends about 220).
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(next((ROOT / "crates/target").glob("*/vsgcc/dfaces"))))
sys.path.insert(0, str(ROOT / "tools"))
import llrmbin  # noqa: E402
import wrapper  # noqa: E402  the d_faces wrapper and flags run.py builds with
# Steps that run other passes; their total is the passes' too.
CONTAINERS = {"pipeline"}


def parsed(text: str) -> list[dict]:
    """The passes of one compile's stderr: name, runs, IR instructions run over, total and own Minstr, instructions per IR
    instruction per run; those with no function to measure by (module and program passes) are left out."""
    sizes = {name: (int(runs), int(size)) for name, runs, size in re.findall(r"^\[counts\] pass (\S+) (\d+) runs (\d+) instructions$", text, re.M)}
    rows = re.findall(r"^\[instr\]\s+([\d.]+) Minstr own\s+([\d.]+) Minstr total\s+\d+x mir (\S+)$", text, re.M)
    found = []
    for own, total, name in rows:
        runs, size = sizes.get(name, (0, 0))
        # `pipeline` is the driver's own step: it holds every pass of the rounds, so it is no pass to price.
        if size and name not in CONTAINERS:
            found.append(
                {"pass": name, "runs": runs, "ir": size, "total": float(total), "own": float(own), "per": float(total) * 1e6 / size}
            )
    return found


def table(passes: list[dict], budget: float) -> str:
    out = [f"{'pass':16} {'runs':>5} {'IR instrs':>10} {'total M':>8} {'own M':>7} {'per IR instr':>12} {'x budget':>9} {'over M':>7}"]
    for one in sorted(passes, key=lambda one: -(one["total"] - budget * one["ir"] / 1e6)):
        over = max(0.0, one["total"] - budget * one["ir"] / 1e6)
        out.append(
            f"{one['pass']:16} {one['runs']:5} {one['ir']:10} {one['total']:8.1f} {one['own']:7.1f} {one['per']:12.0f} "
            f"{one['per'] / budget:9.1f} {over:7.1f}"
        )
    return "\n".join(out)


def command(llrm: str, level: str, source: str | None, qcport: Path, work: Path) -> list[str]:
    """One compile: `source` as given, else QCport's d_faces through the wrapper run.py compiles it with."""
    if source:
        return [llrm, "-c", *llrmbin.c_args(32), f"-{level}", "-o", str(work / "pass-budgets.obj"), source]
    flat = wrapper.write_source(qcport / "render/d_faces.c", work / "d_faces_flat.c")
    return [llrm, "-c", *llrmbin.c_args(32), f"-{level}", *wrapper.includes(qcport), "-o", str(work / "pass-budgets.obj"), str(flat)]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("source", nargs="?", help="a C file; by default d_faces of $QCPORT")
    parser.add_argument("--llrm", default=str(llrmbin.compiler(ROOT / "target/release")))
    parser.add_argument("--levels", default="O1,O2,Os")
    parser.add_argument("--budget", type=float, default=300.0, help="instructions per IR instruction per run (plan step 4.1)")
    args = parser.parse_args()
    qc = Path(os.environ.get("QCPORT", "")).expanduser()
    work = Path(tempfile.mkdtemp(prefix="pass-budgets-"))
    for level in args.levels.split(","):
        done = subprocess.run(
            command(args.llrm, level, args.source, qc, work),
            capture_output=True, text=True, env={**os.environ, **llrmbin.cc_environment(), "LLRM_DEBUG": "counts,time", "LLRM_TIME_TOP": "2000"},
        )
        if done.returncode:
            sys.exit(f"failed at -{level}: {done.stderr[-400:]}")
        print(f"-{level}: {len(parsed(done.stderr))} function passes\n{table(parsed(done.stderr), args.budget)}\n")


if __name__ == "__main__":
    main()
