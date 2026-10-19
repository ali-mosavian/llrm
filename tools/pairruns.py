#!/usr/bin/env python3
"""Output correctness of a rows pair: the FAIL lines of `run_groups.py` for the base and the new binaries, and what the flip changed.

    tools/pairruns.py BASE_RUNS.txt NEW_RUNS.txt      prints `NEW FAIL program [config]: reason` for each failure only in NEW (wrong outputs named as such),
                                                      `FIXED program [config]` for each only in BASE; exit 1 if NEW has a failure BASE lacks

    tools/pairruns.py --tree DIR SHA                  exit 2 unless the checkout in DIR is at SHA: run_groups takes tests/run and the runtime from its tree, not from the binaries

A pair that measures clocks and bytes alone read a flip that miscompiles two programs (JumpThreading on by default: logical_value, compare_to_int64) as a win.
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

FAIL = re.compile(r"^FAIL\s+(\S+)((?: \[[^\]]*\])*): (.*)$")


def failures(text: str) -> dict[str, str]:
    """`program [config] [config]` -> reason, for every FAIL line of a run_groups output."""
    found = {}
    for line in text.splitlines():
        if match := FAIL.match(line):
            found[f"{match[1]}{match[2]}"] = match[3].strip()
    return found


PREMISE_MAX = 10  # a base failing more programs than this is a broken instrument (a missing tool), not a compiler: nothing can be said of the flip


def compare(base: str, new: str) -> tuple[list[str], list[str]]:
    """(lines for the failures only in `new`, names fixed in `new`). A side that fails more than PREMISE_MAX programs makes the comparison void: one line says so."""
    before, after = failures(base), failures(new)
    for side, text in (("base", base), ("new", new)):
        if not re.search(r"^\[\w+\] \d+ programs:", text, re.M):
            return [f"PREMISE BROKEN: the {side} run printed no group line ({(text.strip().splitlines() or ['empty'])[0][:100]}); nothing ran"], []
    for side, found in (("base", before), ("new", after)):
        if len(found) > PREMISE_MAX:
            first = next(iter(found.items()))
            return [f"PREMISE BROKEN: the {side} fails {len(found)} programs (first: {first[0]}: {first[1][:100]}); the comparison says nothing"], []
    added = [f"NEW FAIL {name}: {reason}" for name, reason in after.items() if name not in before]
    return added, [name for name in before if name not in after]


def tree_mismatch(tree: Path, sha: str) -> str:
    """Why `tree` is not at `sha` (empty when it is). A tree at another sha ran the wrong expected outputs against the binaries: planets.nib failed on both sides of a pair and the 11 real failures were buried in it."""
    head = subprocess.run(["git", "-C", str(tree), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    return "" if head and head.startswith(sha) or sha.startswith(head) and head else f"TREE MISMATCH: {tree} is at {head[:9] or 'no commit'}, the binaries are {sha[:9]}; check it out first"


def main() -> int:
    if sys.argv[1] == "--tree":
        why = tree_mismatch(Path(sys.argv[2]), sys.argv[3])
        print(why or "tree matches")
        return 2 if why else 0
    added, fixed = compare(Path(sys.argv[1]).read_text(), Path(sys.argv[2]).read_text())
    print("\n".join(added) or "no new tests/run failure")
    if added and added[0].startswith("PREMISE BROKEN"):
        return 2
    print("\n".join(f"FIXED {name}" for name in fixed))
    return 1 if added else 0


if __name__ == "__main__":
    sys.exit(main())
