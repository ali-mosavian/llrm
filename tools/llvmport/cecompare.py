#!/usr/bin/env python3
"""Compares `constraint-elimination`, the pinned LLVM's against llrm's, on .ll files.

    tools/llvmport/cecompare.py FILE.ll|DIR... [--show N] [--verbose]

A directory is every `.ll` under it. For each file both sides run the pass over every function (`opt -passes=constraint-elimination` of the
pinned build, `examples/ce.rs` of llrm-transforms; CARGO_TARGET_DIR is honoured) and both outputs go through the pinned `opt
-passes=strip` (value names are not part of the answer: LLVM's `takeName` has no counterpart). A file llrm cannot read, or the
pinned opt rejects, is skipped and counted, never compared. Premise: the build's revision must be the pin. Prints files
compared, same and different, and for each different one the first differing line of both sides. Exit 1 on a difference, a wrong
revision or nothing compared.
"""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PIN = "d1106deb71cc81016406a1917d0508d2df296266"
BUILD = Path(os.environ.get("LLVM_BUILD", Path.home() / "scratch/llvm-build"))
OPT = BUILD / "bin/opt"


def run(cmd, stdin=None):
    done = subprocess.run(cmd, input=stdin, capture_output=True, text=True)
    return done.returncode, done.stdout, done.stderr


def strip(text):
    code, out, _ = run([str(OPT), "-passes=strip", "-S", "-o", "-"], text)
    # The module's own names are the file's, not the pass's.
    return "\n".join(l for l in out.splitlines() if not l.startswith(("source_filename", "; ModuleID"))) if code == 0 else None


def first_difference(ours, theirs):
    """The first pair of lines that differ, a missing line as the empty one; None where the texts are the same."""
    a, b = ours.splitlines(), theirs.splitlines()
    return next(((x, y) for x, y in zip(a + [""], b + [""]) if x != y), None)


def main():
    show = int(sys.argv[sys.argv.index("--show") + 1]) if "--show" in sys.argv else 20
    verbose = "--verbose" in sys.argv
    paths = [a for a in sys.argv[1:] if not a.startswith("--") and a != str(show)]
    files = []
    for path in paths:
        files += sorted(str(f) for f in Path(path).rglob("*.ll")) if Path(path).is_dir() else [path]
    revision = (BUILD / "include/llvm/Support/VCSRevision.h").read_text()
    code, out, _ = run([str(OPT), "--version"])
    if code or PIN not in revision or "24.0.0git" not in out:
        sys.exit(f"cecompare: the build at {BUILD} is not the pin {PIN[:12]}: {revision.strip()} {out.splitlines()[:3]}")
    print(f"opt {BUILD / 'bin/opt'}: LLVM 24.0.0git {PIN[:12]}")
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    ours_bin = target / "debug/examples/ce" if (target / "debug/examples/ce").exists() else target / "release/examples/ce"
    same = differ = skipped = 0
    for path in files:
        code, theirs, _ = run([str(OPT), "-passes=constraint-elimination", "-S", path, "-o", "-"])
        if code:
            skipped += 1
            continue
        code, ours, err = run([str(ours_bin), path])
        if code:
            skipped += 1
            if verbose:
                print(f"{path}: llrm failed: {next((l for l in err.splitlines() if l.strip() and not l.startswith('thread') and not l.startswith('note:')), code)[:200]}")
            continue
        ours, theirs = strip(ours), strip(theirs)
        if ours is None:
            skipped += 1
            if verbose:
                print(f"{path}: the pinned opt rejects llrm's output")
            continue
        bad = first_difference(ours, theirs)
        if bad is None:
            same += 1
        else:
            differ += 1
            if differ <= show:
                print(f"{path}\n  llrm: {bad[0]}\n  llvm: {bad[1]}")
    print(f"{same + differ} files compared, {same} same, {differ} differ, {skipped} of {len(files)} skipped")
    return 1 if differ or not (same + differ) else 0


if __name__ == "__main__":
    sys.exit(main())
