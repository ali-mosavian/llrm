#!/usr/bin/env python3
"""Allocate one function's pre-allocation LIR with the pin and with llrm, on the same input.

    check.py FUNCTION FILE.c [llrm-c flags]      # default flags: -m32 -mabi=sysv -march=i486 -O2

llrm-c prints the function as MIR (`LLRM_MIR_PRINT`); the pin's `llc -run-pass=greedy,virtregrewriter` allocates it; the stack
references of the pin's result (spill stores, reloads, argument reads) is counted next to what llrm's spiller recorded for the same input (stores, reloads, folded reloads). Only the
integer forms `backend/mirprint.rs` knows are printed; any other instruction is named and the run stops.
"""
from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
LLC = Path(os.environ.get("LLC") or Path.home() / "scratch/llvm-build/bin/llc")


def llrm_c() -> str:
    return os.environ.get("LLRM_C") or str(REPO / "target/debug/llrm-c")


def spill_counts(stderr: str) -> dict[str, int]:
    """What the spiller made for the function: `SPILL-COUNTS name stores=N reloads=N folded=N`."""
    line = next(line for line in stderr.splitlines() if line.startswith("SPILL-COUNTS"))
    return {key: int(value) for key, value in re.findall(r"(\w+)=(\d+)", line)}


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__)
        return 2
    function, source, flags = argv[0], argv[1], argv[2:] or ["-m32", "-mabi=sysv", "-march=i486", "-O2"]
    done = subprocess.run([llrm_c(), *flags, "-S", "-o", "/dev/stdout", source], capture_output=True, text=True,
                          env={**os.environ, "LLRM_MIR_PRINT": function})
    if "MIR-PRINT-REFUSED" in done.stderr:
        print(next(line for line in done.stderr.splitlines() if "MIR-PRINT-REFUSED" in line))
        return 1
    mir = done.stderr[done.stderr.index("--- |"):done.stderr.index("SPILL-COUNTS")]
    ours = spill_counts(done.stderr)
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "f.mir"
        path.write_text(mir)
        pin = subprocess.run([str(LLC), "-mtriple=i486-unknown-linux-gnu", "-mcpu=i486", "-mattr=-sse,-sse2,-mmx", "-O2",
                              "-start-before=greedy", "-stop-after=virtregrewriter", "-x", "mir", str(path), "-o", "-"],
                             capture_output=True, text=True)
    if pin.returncode:
        print("the pin refused the MIR:", pin.stderr[:600])
        return 1
    stores = len(re.findall(r"MOV32mr %stack\.", pin.stdout))
    reloads = len(re.findall(r"[A-Z0-9]+32rm %stack\.", pin.stdout))
    arguments = len(re.findall(r"%fixed-stack\.", pin.stdout))
    print(f"{function}: pin    stores {stores}, reloads {reloads}, argument {arguments}")
    print(f"{function}: llrm   stores {ours['stores']}, reloads {ours['reloads'] + ours['folded']} ({ours['reloads']} loads + {ours['folded']} folded), "
          f"argument {mir.count('%fixed-stack.')}   (the spiller's own records)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
