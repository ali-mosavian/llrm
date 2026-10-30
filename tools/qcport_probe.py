#!/usr/bin/env python3
"""One compile of the QCport sweep's front-end set, asked whether it fails with a given kind.

    LLRM_BIN=DIR tools/qcport_probe.py PROGRAM MACHINE LEVEL KIND

PROGRAM is a file name from a `NEW:` line (`matrix.bas`), MACHINE `-m32`, LEVEL `-O1`, KIND the kind text the sweep printed
(`panic crates/ir/llrm-mir/src/passes.rs:N:N: lsr added ...`; a prefix is enough). Compiled with the program's flags and the
oracles on, as `sweep_extra` does. Exit 1: it fails with that kind; 0: it compiles or fails otherwise; 3: no such job (so
`sweep_hourly.py` skips the build in a bisect instead of calling it green).
"""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import qcport_sweep as q  # noqa: E402


def probe(
    program: str,
    machine: str,
    level: str,
    wanted: str,
    bins: Path,
    root: Path = q.ROOT,
) -> int:
    jobs = [job for job in q.extra_jobs(root) if job[1].name == program and job[2] == machine and job[3] == level]
    if not jobs:
        return 3
    tool, source, _, _ = jobs[0]
    env = {**os.environ, **{f"LLRM_CHECK_{name}": "1" for name in q.ORACLES}}
    done = subprocess.run([str(bins / tool), str(source), *q.program_flags(source), level, machine, "-o", os.devnull], capture_output=True, text=True, env=env)
    if done.returncode == 0:
        return 0
    return 1 if q.kind(done.stderr or done.stdout).startswith(wanted) else 0


def main(argv: list[str]) -> int:
    if len(argv) != 4:
        print(__doc__)
        return 2
    bins = Path(os.environ.get("LLRM_BIN", ""))
    return probe(argv[0], argv[1], argv[2], argv[3], bins)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
