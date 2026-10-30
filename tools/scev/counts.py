#!/usr/bin/env python3
"""Loop trip counts against the pinned `opt -passes='print<scalar-evolution>'`.

    tools/scev/counts.py [--bin PATH] [FILE.ll ...]    (default: the analysis corpus)

Compares each "Determining loop execution counts" section line for line.
Prints `files total / compared / failed / missing`, then each mismatch. Exit 1
on any mismatch or missing side.
"""
import glob
import subprocess
import sys
from pathlib import Path

PINNED = Path.home() / "scratch/llvm-build/bin/opt"
COMMIT = "24.0.0git"
ROOT = Path(__file__).resolve().parent.parent.parent


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True)


def sections(text):
    """All of them, in order: opt prints each function's classification, then
    its counts."""
    out, current = [], None
    for line in text.splitlines():
        if line.startswith("Determining loop execution counts"):
            current = [line]
            out.append(current)
        elif line.startswith(("Classifying expressions", "Printing analysis")):
            current = None
        elif current is not None and line.strip():
            current.append(line)
    return [l for s in out for l in s]


def main():
    args = sys.argv[1:]
    binary = ROOT / "target/release/llrm-mir"
    if args[:1] == ["--bin"]:
        binary, args = Path(args[1]), args[2:]
    version = run([str(PINNED), "--version"]).stdout
    if COMMIT not in version:
        raise SystemExit(f"counts.py: {PINNED} is not LLVM {COMMIT}")
    files = args or sorted(glob.glob(str(ROOT / "crates/opt/llrm-analysis/corpus/*/*.ll")))
    compared = failed = missing = loops = 0
    bad = []
    for f in files:
        want = run([str(PINNED), "-passes=print<scalar-evolution>", "-disable-output", f])
        got = run([str(binary), "--scev-counts", f])
        if want.returncode or got.returncode:
            missing += 1
            why = want.stderr if want.returncode else got.stderr
            bad.append(f"{f}: opt rc {want.returncode}, llrm-mir rc {got.returncode}: {why[-200:]}")
            continue
        compared += 1
        a, b = sections(want.stderr), [l for l in got.stdout.splitlines() if l.strip()]
        loops += sum(1 for l in a if "backedge-taken count" in l and "constant max" not in l and "symbolic" not in l)
        if a != b:
            failed += 1
            at = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
            bad.append(f"{f}\n  opt  : {a[at] if at < len(a) else '<end>'}\n  llrm : {b[at] if at < len(b) else '<end>'}")
    print(f"files {len(files)} total / {compared} compared / {failed} failed / {missing} missing")
    print(f"loops in opt's output {loops}")
    if not loops:
        bad.append("premise: opt's output holds no loop count")
        failed += 1
    print("\n".join(bad[:30]))
    sys.exit(1 if failed or missing else 0)


main()
