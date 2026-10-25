#!/usr/bin/env python3
"""Merge driver for a tests file two PRs append to: `merge_tests_append.py BASE OURS THEIRS`, the result in OURS, exit 1 on a conflict.

`merge=union` kept both sides' lines but aligned the tail of one appended test with the tail of the other and dropped a closing `}` (#4570: llrm-core did not
compile, no conflict reported). Here a side that only inserted lines at one place in BASE inserts them whole; both sides inserting at the same place give OURS's
block and then THEIRS's. Anything else is the plain three-way merge, conflicts included.
"""

import subprocess
import sys
from pathlib import Path


def insertion(base: list[str], side: list[str]) -> tuple[int, int, list[str]] | None:
    """(first, last, lines): `side` is `base` with `lines` inserted at any index in first..last, or None when it changed anything else."""
    if len(side) <= len(base):
        return None
    prefix = 0
    while prefix < len(base) and base[prefix] == side[prefix]:
        prefix += 1
    suffix = 0
    while suffix < len(base) - prefix and base[-1 - suffix] == side[-1 - suffix]:
        suffix += 1
    low = len(base) - suffix
    if low > prefix:
        return None
    # The suffix match may reach back into the prefix: any index from `low` to `prefix` yields `side`.
    return low, prefix, side[prefix : prefix + len(side) - len(base)]


def merged(base: list[str], ours: list[str], theirs: list[str]) -> list[str] | None:
    mine, other = insertion(base, ours), insertion(base, theirs)
    if mine is None or other is None:
        return None
    at = min(mine[1], other[1])
    if at < max(mine[0], other[0]):
        return None
    # `lines` were taken at `prefix`; at another valid index the same lines read from `side`.
    return base[:at] + ours[at : at + len(ours) - len(base)] + theirs[at : at + len(theirs) - len(base)] + base[at:]


def main() -> int:
    base_path, ours_path, theirs_path = sys.argv[1:4]
    lines = lambda path: Path(path).read_text().splitlines(keepends=True)
    base, ours, theirs = lines(base_path), lines(ours_path), lines(theirs_path)
    plain = subprocess.run(["git", "merge-file", "-p", ours_path, base_path, theirs_path], capture_output=True, text=True)
    if plain.returncode == 0:
        Path(ours_path).write_text(plain.stdout)
        return 0
    both = merged(base, ours, theirs)
    if both is None:
        Path(ours_path).write_text(plain.stdout)
        return 1
    Path(ours_path).write_text("".join(both))
    return 0


if __name__ == "__main__":
    sys.exit(main())
