#!/usr/bin/env python3
"""First bad first-parent commit between GOOD and BAD for a command that passes (exit 0) or fails (exit 1), on a dist build of each probed commit.

    PAIR_WT=<worktree> tools/pairs/firstbad.py GOOD BAD -- CMD...        CMD runs in PAIR_WT at the probed commit with DIST=<dir of its llrm-c, llrm-nib, jwasm, dosbox-x>

Any other exit code stops the run: a runner that refused to run (stale binaries, no summary) is not a failing commit. Both ends are probed first: a GOOD that fails or a BAD that passes stops the run (the predicate is not the bug). Builds go to SCRATCH/sbp-SHA9 (pair_build.sh, one
target dir per worktree) and are reused; a probe that does not build counts as skipped and the search moves up. Prints `FIRST BAD sha subject`.
"""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRATCH = Path(os.environ.get("PAIR_SCRATCH", Path.home() / "scratch"))
WORKTREE = Path(os.environ["PAIR_WT"]) if "PAIR_WT" in os.environ else None
cache: dict[str, bool | None] = {}


def git(*args: str) -> str:
    return subprocess.run(["git", *args], cwd=WORKTREE, capture_output=True, text=True, check=True).stdout.strip()


def probe(sha: str, cmd: list[str]) -> bool | None:
    """True: the command passes at `sha`; False: fails; None: no dist build."""
    s9 = sha[:9]
    if s9 in cache:
        return cache[s9]
    git("checkout", "-q", "--detach", sha)
    built = subprocess.run(["bash", str(HERE / "pair_build.sh"), str(SCRATCH), s9], cwd=WORKTREE, capture_output=True, text=True)
    dist = SCRATCH / f"sbp-{s9}" / "dist"
    if built.returncode or not (dist / "llrm-c").exists():
        cache[s9] = None
    else:
        for name in dist.glob("llrm-*"):  # the runners refuse binaries older than the checked-out sources; a copy made at build time keeps the old time
            name.touch()
        (dist / "nibfront").touch()
        result = subprocess.run(cmd, cwd=WORKTREE, env={**os.environ, "DIST": str(dist)}, capture_output=True, text=True)
        if result.returncode not in (0, 1):
            raise SystemExit(f"probe {s9}: the command exited {result.returncode}, not 0 or 1: {(result.stdout + result.stderr).strip()[-300:]}")
        cache[s9] = result.returncode == 0
    print(f"probe {s9}: {'builds' if cache[s9] is not None else 'DOES NOT BUILD'}, {'pass' if cache[s9] else 'fail'}", flush=True)
    return cache[s9]


def main() -> int:
    if WORKTREE is None or "--" not in sys.argv:
        print(__doc__)
        return 2
    cut = sys.argv.index("--")
    good, bad, cmd = sys.argv[1], sys.argv[2], sys.argv[cut + 1:]
    commits = git("rev-list", "--first-parent", "--reverse", f"{good}..{bad}").split()
    if probe(good, cmd) is not True or probe(commits[-1], cmd) is not False:
        print(f"the predicate does not separate the ends: good {cache.get(good[:9])}, bad {cache.get(commits[-1][:9])}")
        return 2
    lo, hi = -1, len(commits) - 1
    while hi - lo > 1:
        mid = (lo + hi) // 2
        got = probe(commits[mid], cmd)
        if got is None:
            lo = mid
        elif got:
            lo = mid
        else:
            hi = mid
    print("FIRST BAD", commits[hi][:9], git("log", "-1", "--format=%s", commits[hi]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
