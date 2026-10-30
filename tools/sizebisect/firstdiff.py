#!/usr/bin/env python3
"""The first MIR stage where two builds' bodies differ, in the order the compile ran.

    tools/sizebisect/firstdiff.py BASE_STAGES NEW_STAGES [LINES]

Each argument is a `LLRM_MIR_STAGES=<dir>` dump of one compile: `NN/` holds the steps of pipeline run NN (a step is written only
where it changed the body), `NN-name.ll` is the module after it. Value numbers are normalised. Steps that exist in only one
build count: a new slot's own step is where the two bodies part (the first `difference` of the old fr2 script skipped them, and
named a form that came after as the cause).
"""
from __future__ import annotations

import difflib
import glob
import os
import re
import sys


def normalised(text: str) -> str:
    return re.sub(r"label %\d+", "label %N", re.sub(r"%\d+", "%N", text))


def order(path: str, root: str) -> tuple[int, int, str]:
    """The compile's order: the steps of run N, then the snapshot after it."""
    parts = os.path.relpath(path, root).split(os.sep)
    head = parts[0].split("-")[0]
    number = int(head) if head.isdigit() else 999
    return (number, 1, parts[0]) if len(parts) == 1 else (number, 0, os.sep.join(parts[1:]))


def stages(root: str) -> list[str]:
    return sorted(glob.glob(root + "/**/*.ll", recursive=True), key=lambda path: order(path, root))


def first_difference(base: str, new: str, lines: int = 40) -> tuple[str, str] | None:
    """(stage, normalised diff) of the first stage that differs or exists in one build only; None if all agree."""
    both = {os.path.relpath(path, root) for root in (base, new) for path in stages(root)}
    for name in sorted(both, key=lambda one: order(os.path.join(base, one), base)):
        a, b = os.path.join(base, name), os.path.join(new, name)
        if not (os.path.exists(a) and os.path.exists(b)):
            return name, "present in the " + ("new" if os.path.exists(b) else "base") + " build only"
        left, right = normalised(open(a).read()), normalised(open(b).read())
        if left != right:
            diff = list(difflib.unified_diff(left.split("\n"), right.split("\n"), lineterm="", n=1))[2:lines]
            return name, "\n".join(line[:150] for line in diff)
    return None


def main() -> None:
    found = first_difference(sys.argv[1], sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 40)
    print("all dumped stages equal" if found is None else f"first differing stage: {found[0]}\n{found[1]}")


if __name__ == "__main__":
    main()
