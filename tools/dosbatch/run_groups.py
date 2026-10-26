#!/usr/bin/env python3
"""tests/run, examples and bench by group (the directory a program is in), one DOSBox launch per group.

    python3 tools/dosbatch/run_groups.py [GROUP...]

One launch for all 848 programs stops at the first program that floods events.txt (33 MB) and reports the 262 that follow as failed: a run that ends there says
nothing about them. A group of its own contains such a program. Prints each group's line and every FAIL line; exit 1 if any program failed or a group's launch was stopped.
"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import run_tests  # noqa: E402


def groups() -> dict[str, int]:
    """group -> programs in it, in the order of the full run."""
    out: dict[str, int] = {}
    for program in run_tests.discover([]):
        out[program.source.parent.name] = out.get(program.source.parent.name, 0) + 1
    return out


def main() -> int:
    chosen = sys.argv[1:] or list(groups())
    failed = stopped = 0
    for group in chosen:
        done = subprocess.run([sys.executable, str(HERE / "run_tests.py"), group], capture_output=True, text=True)
        lines = (done.stdout + done.stderr).splitlines()
        summary = next((line for line in reversed(lines) if " programs: " in line), f"{group}: no summary (rc {done.returncode})")
        bad = [line for line in lines if line.startswith("FAIL")]
        proof = [line for line in lines if line.startswith("isel proof")]
        flood = [line for line in bad if "launch was stopped" in line]
        failed += len(bad)
        stopped += bool(flood)
        print(f"[{group}] {summary}" + (f"  LAUNCH STOPPED ({len(flood)} programs unjudged)" if flood else ""))
        print("\n".join(proof + bad))
    return 1 if failed or stopped else 0


if __name__ == "__main__":
    sys.exit(main())
