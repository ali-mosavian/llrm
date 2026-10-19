#!/usr/bin/env python3
"""Output correctness of a rows pair: the FAIL lines of `run_groups.py` for the base and the new binaries, and what the flip changed.

    tools/pairruns.py BASE_RUNS.txt NEW_RUNS.txt      prints `NEW FAIL program [config]: reason` for each failure only in NEW (wrong outputs named as such),
                                                      `FIXED program [config]` for each only in BASE; exit 1 if NEW has a failure BASE lacks

A pair that measures clocks and bytes alone read a flip that miscompiles two programs (JumpThreading on by default: logical_value, compare_to_int64) as a win.
"""
from __future__ import annotations

import re
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


def compare(base: str, new: str) -> tuple[list[str], list[str]]:
    """(lines for the failures only in `new`, names fixed in `new`)."""
    before, after = failures(base), failures(new)
    added = [f"NEW FAIL {name}: {reason}" for name, reason in after.items() if name not in before]
    return added, [name for name in before if name not in after]


def main() -> int:
    added, fixed = compare(Path(sys.argv[1]).read_text(), Path(sys.argv[2]).read_text())
    print("\n".join(added) or "no new tests/run failure")
    print("\n".join(f"FIXED {name}" for name in fixed))
    return 1 if added else 0


if __name__ == "__main__":
    sys.exit(main())
