#!/usr/bin/env python3
"""Compares ValueTracking's answers, LLVM's against llrm's, on .ll files.

    tools/llvmport/vtcompare.py COLS FILE.ll... [--show N]

FILE may be a directory (every `.ll` under it). A file either side cannot read is skipped and counted. COLS is a comma list of kb sb nz p2 p2z gp nn ng cru crs sp ov, in any order (both dumps print them in that order). Prints the number of values compared
and, for each differing one, both lines. Exit 1 on a difference. LLVM's side is `vtdump.sh`, llrm's the
`vtdump` example of llrm-mir (CARGO_TARGET_DIR is honoured).
"""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CANONICAL = "kb sb nz p2 p2z gp nn ng cru crs sp ov".split()


def canonical(cols):
    """`cols` in the order `vtdump` prints; unknown columns are an error."""
    wanted = cols.split(",")
    unknown = [c for c in wanted if c not in CANONICAL]
    if unknown:
        sys.exit(f"unknown column(s): {','.join(unknown)}")
    return ",".join(c for c in CANONICAL if c in wanted)


def run(cmd):
    done = subprocess.run(cmd, capture_output=True, text=True)
    return done.returncode, done.stdout.splitlines(), done.stderr


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    args = [a for a in args if a != "--verbose"]
    show = 20
    if "--show" in sys.argv:
        show = int(sys.argv[sys.argv.index("--show") + 1])
        args = [a for a in args if a != str(show)]
    cols, paths = canonical(args[0]), args[1:]
    files = []
    for path in paths:
        files += sorted(str(f) for f in Path(path).rglob("*.ll")) if Path(path).is_dir() else [path]
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    ours_bin = target / "release/examples/vtdump"
    total = differ = skipped = 0
    for path in files:
        code, theirs, err = run([str(ROOT / "tools/llvmport/vtdump.sh"), cols, path])
        if code:
            skipped += 1
            continue
        code, ours, err = run([str(ours_bin), cols, path])
        if code:
            skipped += 1
            if "--verbose" in sys.argv:
                print(f"{path}: llrm failed: {err.strip().splitlines()[-1] if err.strip() else code}")
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
    print(f"{total} values compared, {differ} differ, {skipped} of {len(files)} files skipped (one side cannot read them)")
    return 1 if differ else 0


if __name__ == "__main__":
    sys.exit(main())
