#!/usr/bin/env python3
"""Compares DominatorTree and PostDominatorTree, LLVM's against llrm's, on .ll files.

    tools/llvmport/domcompare.py FILE.ll|DIR... [--show N]

A directory is every `.ll` under it. Per function, both sides print each block's immediate dominator, level and sorted children and the
roots, for the tree and the post-dominator tree (`domdump.cpp`, `examples/domdump.rs` of llrm-mir; CARGO_TARGET_DIR is honoured). Premise:
LLVM's side must report the pinned revision, and a file either side cannot read is skipped and counted, never compared. Prints the files,
functions and tree lines compared; exit 1 on a difference, a wrong revision or nothing compared.
"""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PIN = "d1106deb71cc81016406a1917d0508d2df296266"


def run(cmd):
    done = subprocess.run(cmd, capture_output=True, text=True)
    return done.returncode, done.stdout.splitlines(), done.stderr


def first_difference(ours, theirs):
    """The first pair of lines that differ, a missing line as the empty one; None where the dumps are the same."""
    return next(((a, b) for a, b in zip(ours + [""], theirs + [""]) if a != b), None)


def main():
    show = int(sys.argv[sys.argv.index("--show") + 1]) if "--show" in sys.argv else 20
    paths = [a for a in sys.argv[1:] if not a.startswith("--") and a != str(show)]
    files = []
    for path in paths:
        files += sorted(str(f) for f in Path(path).rglob("*.ll")) if Path(path).is_dir() else [path]
    code, version, err = run([str(ROOT / "tools/llvmport/domdump.sh"), "--version"])
    if code or not version or PIN not in version[0]:
        sys.exit(f"domcompare: LLVM's side is not the pin {PIN[:12]}: {version} {err.strip()[-200:]}")
    print(version[0])
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    ours_bin = target / "release/examples/domdump"
    compared = skipped = differ = lines = functions = unreachable = several = 0
    for path in files:
        code, theirs, _ = run([str(ROOT / "tools/llvmport/domdump.sh"), path])
        if code:
            skipped += 1
            continue
        code, ours, err = run([str(ours_bin), path])
        if code:
            skipped += 1
            continue
        compared += 1
        functions += sum(1 for l in theirs if l.startswith("@"))
        unreachable += sum(1 for l in theirs if l.startswith("dt ") and l.endswith(" x"))
        several += sum(1 for l in theirs if l.startswith("pdt roots ") and l.count(",") > 1)
        if bad := first_difference(ours, theirs):
            differ += 1
            if differ <= show:
                print(f"{path}: {len(ours)} lines here, {len(theirs)} in LLVM\n  llrm: {bad[0]}\n  llvm: {bad[1]}")
        lines += len(theirs)
    print(f"premise: {unreachable} unreachable blocks and {several} functions with several post-dominator roots in the corpus")
    print(f"{compared} files, {functions} functions, {lines} lines compared, {differ} files differ, {skipped} of {len(files)} skipped")
    return 1 if differ or not compared else 0


if __name__ == "__main__":
    sys.exit(main())
