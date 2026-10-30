#!/usr/bin/env python3
"""Merge driver for a tests file two PRs append to: `merge_tests_append.py BASE OURS THEIRS`, the result in OURS, exit 1 on a conflict.

`merge=union` kept both sides' lines but aligned the tail of one appended test with the tail of the other and dropped a closing `}` (#4570: llrm-codegen did not
compile, no conflict reported). Here the plain three-way merge runs, and a conflict hunk where BASE has nothing (both sides inserted at one place) becomes OURS's
block and then THEIRS's, each whole, unless both blocks define the same item (a test two PRs add alike would be defined twice). Any other conflict stays one: the result keeps the markers and the exit is 1. Edits elsewhere in the file merge as git merges them.
"""

import re
import subprocess
import sys
from pathlib import Path


ITEM = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+|unsafe\s+|async\s+)?(fn|static|struct|enum|type|mod|trait|const)\s+(\w+)", re.M)


def defined(lines: list[str]) -> set[tuple[str, str]]:
    return set(ITEM.findall("".join(lines)))


def resolved(diff3: str) -> str | None:
    """`diff3` (a `git merge-file --diff3` output) with every empty-base conflict joined; None when a conflict has anything in its base."""
    out: list[str] = []
    state, ours, base, theirs = "text", [], [], []
    for line in diff3.splitlines(keepends=True):
        if state == "text" and line.startswith("<<<<<<< "):
            state, ours, base, theirs = "ours", [], [], []
        elif state == "ours" and line.startswith("||||||| "):
            state = "base"
        elif state == "base" and line.startswith("======="):
            state = "theirs"
        elif state == "theirs" and line.startswith(">>>>>>> "):
            if base or defined(ours) & defined(theirs):
                return None  # something in the base, or both sides defined the same item (two PRs adding one test: joined, it would be defined twice)
            out += ours + (["\n"] if ours and not ours[-1].endswith("\n") else []) + theirs
            state = "text"
        elif state == "ours":
            ours.append(line)
        elif state == "base":
            base.append(line)
        elif state == "theirs":
            theirs.append(line)
        else:
            out.append(line)
    return None if state != "text" else "".join(out)


def main() -> int:
    base, ours, theirs = sys.argv[1:4]
    merge = lambda *flags: subprocess.run(["git", "merge-file", "-p", *flags, ours, base, theirs], capture_output=True, text=True)  # noqa: E731
    both = merge("--diff3")
    if both.returncode == 0:
        Path(ours).write_text(both.stdout)
        return 0
    joined = resolved(both.stdout)
    if joined is not None:
        Path(ours).write_text(joined)
        return 0
    Path(ours).write_text(merge().stdout)
    return 1


if __name__ == "__main__":
    sys.exit(main())
