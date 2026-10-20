#!/usr/bin/env python3
"""What the flip changed in tests/run, with the programs a side could not build set aside by name.

    tools/pairs/rundiff.py BASE_RUNS.txt NEW_RUNS.txt      `runpinned.sh` outputs; exit 1 on a NEW FAIL, 2 when a side printed no group line

tools/pairruns.py calls a side that fails more than 10 programs a broken instrument. On a VM every program that needs V:\\LINK (qb, one per bench group)
fails with `not built`, 130 of 137: those are counted and named apart (the same on both sides), and the rest is compared as pairruns does.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import pairruns  # noqa: E402


def split(text: str) -> tuple[dict[str, str], dict[str, str]]:
    """(failures that ran, failures that were not built)."""
    ran, unbuilt = {}, {}
    for name, reason in pairruns.failures(text).items():
        (unbuilt if reason.startswith("not built") else ran)[name] = reason
    return ran, unbuilt


def compare(base: str, new: str) -> tuple[list[str], list[str], str]:
    """(NEW FAIL lines, fixed names, a one-line census)."""
    for side, text in (("base", base), ("new", new)):
        if not re.search(r"^\[\w+\] \d+ programs:", text, re.M):
            return [f"PREMISE BROKEN: the {side} run printed no group line"], [], ""
    (ran_a, skip_a), (ran_b, skip_b) = split(base), split(new)
    added = [f"NEW FAIL {name}: {reason}" for name, reason in ran_b.items() if name not in ran_a]
    added += [f"NEW NOT BUILT {name}: {reason}" for name, reason in skip_b.items() if name not in skip_a]
    fixed = [name for name in ran_a if name not in ran_b]
    return added, fixed, f"failing programs that ran: base {len(ran_a)}, new {len(ran_b)}; not built (V:\\LINK etc.): base {len(skip_a)}, new {len(skip_b)}"


def main() -> int:
    added, fixed, census = compare(Path(sys.argv[1]).read_text(), Path(sys.argv[2]).read_text())
    print("\n".join(added) or "no new failure")
    print(census)
    if fixed:
        print("FIXED " + ", ".join(fixed))
    return 2 if added and added[0].startswith("PREMISE") else 1 if added else 0


if __name__ == "__main__":
    sys.exit(main())
