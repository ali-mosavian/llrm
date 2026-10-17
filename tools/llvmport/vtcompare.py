#!/usr/bin/env python3
"""Compares ValueTracking's answers, LLVM's against llrm's, on .ll files.

    tools/llvmport/vtcompare.py COLS FILE.ll... [--show N]

COLS is the comma list `vtdump` takes (kb sb nz p2 p2z nn ng cru crs). Prints the number of values compared
and, for each differing one, both lines. Exit 1 on a difference. LLVM's side is `vtdump.sh`, llrm's the
`vtdump` example of llrm-mir (CARGO_TARGET_DIR is honoured).
"""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def run(cmd):
    done = subprocess.run(cmd, capture_output=True, text=True)
    return done.returncode, done.stdout.splitlines(), done.stderr


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    show = 20
    if "--show" in sys.argv:
        show = int(sys.argv[sys.argv.index("--show") + 1])
        args = [a for a in args if a != str(show)]
    cols, files = args[0], args[1:]
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    ours_bin = target / "release/examples/vtdump"
    total = differ = 0
    for path in files:
        code, theirs, err = run([str(ROOT / "tools/llvmport/vtdump.sh"), cols, path])
        if code:
            print(f"{path}: LLVM failed: {err.strip().splitlines()[-1] if err.strip() else code}")
            continue
        code, ours, err = run([str(ours_bin), cols, path])
        if code:
            print(f"{path}: llrm failed: {err.strip().splitlines()[-1] if err.strip() else code}")
            differ += 1
            continue
        if len(ours) != len(theirs):
            print(f"{path}: {len(ours)} values here, {len(theirs)} in LLVM")
            differ += 1
            continue
        for a, b in zip(ours, theirs):
            total += 1
            if a != b:
                differ += 1
                if differ <= show:
                    print(f"{path}\n  llrm: {a}\n  llvm: {b}")
    print(f"{total} values compared, {differ} differ")
    return 1 if differ else 0


if __name__ == "__main__":
    sys.exit(main())
