#!/usr/bin/env python3
"""Whether a CI run's verdict line lets a pull request merge: the merge loop (cimerge.sh) calls this, nothing else judges it.

    candidate_ok.py HEAD MAIN LINE      exit 0 if valid, 1 with the reason on stdout

A merge needs the candidate run, the full gate that runs only for the pull request labelled merge-candidate (docs/working/ci.md): `GATE <tier> CANDIDATE PASS at <head12> on <main12>`.
Any other line (a push to main's `PASS at`, a FAIL, a STALE) never lets a merge. Valid only if the run tested the pull request's head as it is
now (a push after the label made the run stale), and main as it is now, or main moved by commits that touch only [inert] paths (tiers.toml).
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "gate"))
import gate  # noqa: E402

CANDIDATE = re.compile(r"GATE \S+ CANDIDATE PASS at ([0-9a-f]{12}) on ([0-9a-f]{12})\b")


def check(line: str, head: str, main: str, changed: list[str], inert: list[str]) -> tuple[bool, str]:
    """(valid, why). `changed`: the files of the commits from the run's main to main now."""
    found = CANDIDATE.search(line)
    if not found:
        return False, "not a candidate PASS line (a main push's PASS, a FAIL or a STALE line never lets a merge)"
    tested_head, tested_main = found.groups()
    if tested_head != head[:12]:
        return False, f"the run tested {tested_head}, the head is {head[:12]} now: a push after the label; re-run"
    if tested_main != main[:12]:
        live = [f for f in changed if not gate.matches(f, inert)]
        if live:
            return False, f"main moved {tested_main} -> {main[:12]} by {len(live)} non-inert file(s), e.g. {live[0]}: re-run"
    return True, "valid"


def main() -> int:
    head, current, line = sys.argv[1:4]
    found = CANDIDATE.search(line)
    changed: list[str] = []
    if found and found[2] != current[:12]:
        done = subprocess.run(["git", "diff", "--name-only", found[2], current], cwd=ROOT, capture_output=True, text=True)
        changed = done.stdout.split() if done.returncode == 0 else ["<unreadable diff>"]
    ok, why = check(line, head, current, changed, gate.load()["inert"]["paths"])
    print(why)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
