#!/usr/bin/env python3
"""The compile board: d_faces compile cost, one flag set, perf instructions:u, minimum of 3, for several llrm-c builds and the pinned clang.

    QCPORT=<qb-quake src> tools/pairs/board.py --clang CLANG NAME=LLRM_C... [--levels O1,O2,O3,Os]

Every llrm build is measured with and without -fno-inline-functions, and so is clang, at each level; -m32 -march=i486, an object written to a file, the Borland
keywords defined away. A compile that fails stops the run (the count of a failed compile is a few million, not a measurement). For each llrm build the
phases of `LLRM_DEBUG=time` (own Minstr, a step's analyses under their own names) follow, when the build prints them.
"""
from __future__ import annotations

import collections
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

KEYWORDS = [f"-D{k}=" for k in "far huge near pascal cdecl __far _far __near _near".split()]
REGALLOC = {"regalloc", "split", "facts", "spill", "classes", "after", "recolor"}


def run(cmd: list[str], env: dict | None = None) -> subprocess.CompletedProcess:
    done = subprocess.run(cmd, capture_output=True, text=True, env={**os.environ, **(env or {})})
    if done.returncode:
        sys.exit(f"failed: {' '.join(cmd[:4])}\n{done.stderr[-400:]}")
    return done


def instructions(cmd: list[str]) -> float:
    """Minimum of three runs, in M."""
    best = None
    for _ in range(3):
        err = run(["perf", "stat", "-x,", "-e", "instructions:u", *cmd]).stderr
        found = re.search(r"^(\d+),,instructions", err, re.M)
        assert found, err[-200:]
        best = int(found[1]) if best is None else min(best, int(found[1]))
    return best / 1e6


def phases(llrm: str, level: str, args: list[str]) -> dict[str, float]:
    err = run([llrm, "-m32", "-march=i486", level, *args], {"LLRM_DEBUG": "time", "LLRM_TIME_TOP": "700"}).stderr
    if "[instr] by own work" not in err:
        return {}
    own: dict[str, float] = collections.Counter()
    for line in err.split("[instr] by own work")[1].split("nesting")[0].splitlines():
        found = re.search(r"([\d.]+) Minstr own\s+[\d.]+ Minstr total\s+\d+x (\S+)", line)
        if found:
            own["regalloc" if found[2] in REGALLOC else found[2]] += float(found[1])
    return dict(own)


def main() -> int:
    argv = sys.argv[1:]
    if "--clang" not in argv or "QCPORT" not in os.environ:
        print(__doc__)
        return 2
    clang = argv[argv.index("--clang") + 1]
    levels = argv[argv.index("--levels") + 1].split(",") if "--levels" in argv else ["O1", "O2", "O3", "Os"]
    builds = [a.split("=", 1) for a in argv if "=" in a and not a.startswith("-")]
    qc = Path(os.environ["QCPORT"])
    includes = [f for d in ("", "render", "game", "host", "qgl") for f in ("-I", str(qc / d))]
    source = str(qc / "render/d_faces.c")
    sink = os.path.join(tempfile.mkdtemp(prefix="board-"), "out.o")
    print(f"d_faces compile, M instructions:u (min of 3), -m32 -march=i486; clang {run([clang, '--version']).stdout.splitlines()[0]}")
    header = ["level", *(f"{n} {v}" for n, _ in builds for v in ("inline", "noinl")), "clang inline", "clang noinl"]
    print(" | ".join(header))
    table = {}
    for level in levels:
        row = {}
        for name, llrm in builds:
            for tag, extra in (("inline", []), ("noinl", ["-fno-inline-functions"])):
                row[f"{name} {tag}"] = instructions([llrm, "-m32", "-mabi=sysv", "-march=i486", f"-{level}", *extra, *KEYWORDS, *includes, "-o", sink, source])
        for tag, extra in (("inline", []), ("noinl", ["-fno-inline-functions"])):
            row[f"clang {tag}"] = instructions([clang, "-w", "-m32", "-march=i486", f"-{level}", *extra, *KEYWORDS, *includes, "-c", "-o", sink, source])
        table[level] = row
        print(f"-{level} | " + " | ".join(f"{row[h]:.0f}" for h in header[1:]), flush=True)
    for level in levels:
        for name, llrm in builds:
            own = phases(llrm, f"-{level}", [*KEYWORDS, *includes, "-o", sink, source])
            top = sorted(own.items(), key=lambda kv: -kv[1])[:8]
            print(f"-{level} {name} phases (own M): " + (", ".join(f"{k} {v:.0f}" for k, v in top) or "not printed by this build"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
