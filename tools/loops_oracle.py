#!/usr/bin/env python3
"""LoopInfo against the pinned `opt -passes='print<loops>'` on the same IR.

    tools/loops_oracle.py [FILE.ll ...]    (default: the analysis corpus)

Prints `files total / compared / failed / missing` first, then each mismatch.
Exit 1 if any file fails or is missing a side. The pinned opt is named in
one place (`llvmpin` once it lands on main); a different version is refused.
"""
import subprocess, sys, glob
import targetdesc
from pathlib import Path

PINNED = Path.home() / "scratch/llvm-build/bin/opt"
TARGET = targetdesc.triple_cpu()  # llrm's default target (crates/target/*/datalayout.toml): a dump has no `target triple`, and opt costs for x86-64 without one
COMMIT = "24.0.0git"
ROOT = Path(__file__).resolve().parent.parent
LLRM = ROOT / "target/release/llrm-mir"


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True)


def main():
    version = run([str(PINNED), "--version"]).stdout
    if COMMIT not in version:
        raise SystemExit(f"loops_oracle: {PINNED} is not LLVM {COMMIT}")
    print([l.strip() for l in version.splitlines() if "version" in l][0])
    files = sys.argv[1:] or sorted(glob.glob(str(ROOT / "crates/opt/llrm-analysis/corpus/*/*.ll")))
    compared = failed = missing = 0
    loops = nested = 0
    bad = []
    for f in files:
        want = run([str(PINNED), *TARGET, "-passes=print<loops>", "-disable-output", f])
        got = run([str(LLRM), "--loops", f])
        if want.returncode or got.returncode:
            missing += 1
            bad.append(f"{f}: opt rc {want.returncode}, llrm-mir rc {got.returncode}: {(want.stderr if want.returncode else got.stderr)[:200]}")
            continue
        compared += 1
        loops += want.stderr.count("Loop at depth")
        nested += want.stderr.count("Loop at depth 2")
        if want.stderr != got.stdout:
            failed += 1
            a, b = want.stderr.splitlines(), got.stdout.splitlines()
            at = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
            bad.append(f"{f}: line {at}\n  opt  : {a[at][:300] if at < len(a) else ''}\n  llrm : {b[at][:300] if at < len(b) else ''}")
    print(f"files {len(files)} total / {compared} compared / {failed} failed / {missing} missing")
    print(f"opt's trees: {loops} loops, {nested} at depth 2")
    if not nested:
        # An oracle that finds no nesting says nothing about the nesting.
        bad.append("premise: opt's output holds no nested loop, so nothing here tests the tree")
        failed += 1
    print("\n".join(bad[:20]))
    sys.exit(1 if failed or missing else 0)


main()
