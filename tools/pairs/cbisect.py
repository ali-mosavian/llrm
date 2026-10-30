#!/usr/bin/env python3
"""Compile cost of d_faces (perf instructions:u, min of 3; -m32 -mabi=sysv -march=i486; Borland keywords defined away) at -O1 and -O2 for every STEP-th first-parent commit OLD..NEW and both ends.

    QCPORT=<qb-quake src> PAIR_WT=<worktree> tools/pairs/cbisect.py OLD NEW [STEP]     (default STEP 8; ~30 min per probe)

Builds each probe with pair_build.sh in PAIR_WT (SCRATCH/sbp-SHA9), appends one JSON line per commit to SCRATCH/cbisect.jsonl (SCRATCH defaults to ~/scratch)
and skips the commits already there. A failed build is a line with `error`, never a number. Steps of 2% or more are the ones to refine by hand with a smaller STEP.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRATCH = Path(os.environ.get("PAIR_SCRATCH", Path.home() / "scratch"))
WORKTREE = Path(os.environ["PAIR_WT"]) if "PAIR_WT" in os.environ else None
OUT = SCRATCH / "cbisect.jsonl"
KEYWORDS = "far huge near pascal cdecl __far _far __near _near".split()
LEVELS = ("O1", "O2")


def git(*args: str, cwd: Path) -> str:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout.strip()


def perf(cmd: list[str], env: dict) -> int:
    done = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", *cmd], capture_output=True, text=True, env=env)
    match = re.search(r"^(\d+),,instructions", done.stderr, re.M)
    assert done.returncode == 0 and match, f"compile failed: {done.stderr[-300:]}"  # the count of a failed compile is a few million, not a measurement
    return int(match[1])


def measure(sha: str, source: Path, flags: list[str], done: set[str]) -> None:
    s9 = sha[:9]
    if s9 in done:
        return
    built = subprocess.run(["bash", str(HERE / "pair_build.sh"), str(SCRATCH), s9], cwd=WORKTREE, capture_output=True, text=True)
    binary = SCRATCH / f"sbp-{s9}" / "dist" / "llrm-c"
    if built.returncode or not binary.exists():
        record = {"sha": s9, "error": "build failed", "tail": built.stderr[-200:]}
    else:
        env = {k: v for k, v in os.environ.items() if not k.startswith("LLRM_")}
        with tempfile.TemporaryDirectory() as tmp:
            wrapper = Path(tmp) / "d_faces_flat.c"
            wrapper.write_text("".join(f"#define {k}\n" for k in KEYWORDS) + f'#include "{source}"\n')
            cost = {}
            for level in LEVELS:
                cmd = [str(binary), "-m32", "-mabi=sysv", "-march=i486", f"-{level}", *flags, "-o", str(Path(tmp) / "o.obj"), str(wrapper)]
                cost[level] = min(perf(cmd, env) for _ in range(3)) / 1e6
        record = {"sha": s9, "md5": hashlib.md5(binary.read_bytes()).hexdigest()[:12], "subject": git("log", "-1", "--format=%s", sha, cwd=WORKTREE), **cost}
    with OUT.open("a") as handle:
        handle.write(json.dumps(record) + "\n")
    done.add(s9)
    print(record, flush=True)


def main() -> int:
    if WORKTREE is None or "QCPORT" not in os.environ:
        print("set PAIR_WT (a worktree to build in) and QCPORT (the qb-quake source)")
        return 2
    qc = Path(os.environ["QCPORT"])
    flags = [f for d in ("", "render", "game", "host", "qgl") for f in ("-I", str(qc / d if d else qc))]
    done = {json.loads(line)["sha"] for line in OUT.read_text().splitlines()} if OUT.exists() else set()
    old, new = sys.argv[1], sys.argv[2]
    step = int(sys.argv[3]) if len(sys.argv) > 3 else 8
    commits = git("rev-list", "--first-parent", "--reverse", f"{old}^..{new}", cwd=WORKTREE).split()
    picked = commits[::step]
    if commits[-1] not in picked:
        picked.append(commits[-1])
    print(len(commits), "commits;", len(picked), "probes", flush=True)
    for sha in picked:
        measure(sha, qc / "render/d_faces.c", flags, done)
    return 0


if __name__ == "__main__":
    sys.exit(main())
