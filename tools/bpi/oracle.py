#!/usr/bin/env python3
"""Branch probabilities against the pinned `opt -passes='print<branch-prob>'`.

    tools/bpi/oracle.py [--bin PATH] [FILE.ll ...]    (default: the analysis corpus)

Compares each function's edge lines. Prints `files total / compared / failed /
missing`, then each mismatch. Exit 1 on any mismatch or missing side.
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


def lines(text):
    return [l.rstrip() for l in text.splitlines() if "edge %" in l or l.startswith("Printing analysis")]


def main():
    args = sys.argv[1:]
    binary = ROOT / "target/release/llrm-mir"
    if args[:1] == ["--bin"]:
        binary, args = Path(args[1]), args[2:]
    if COMMIT not in run([str(PINNED), "--version"]).stdout:
        raise SystemExit(f"oracle.py: {PINNED} is not LLVM {COMMIT}")
    files = args or sorted(glob.glob(str(ROOT / "crates/opt/llrm-analysis/corpus/*/*.ll")))
    compared = failed = missing = edges = 0
    bad = []
    for f in files:
        want = run([str(PINNED), "-passes=print<branch-prob>", "-disable-output", f])
        got = run([str(binary), "--branch-prob", f])
        if want.returncode or got.returncode:
            missing += 1
            why = want.stderr if want.returncode else got.stderr
            bad.append(f"{f}: opt rc {want.returncode}, llrm-mir rc {got.returncode}: {why[-200:]}")
            continue
        compared += 1
        a, b = lines(want.stderr), lines(got.stdout)
        edges += sum(1 for l in a if "edge %" in l)
        if a != b:
            failed += 1
            at = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
            bad.append(f"{f}\n  opt  : {a[at] if at < len(a) else '<end>'}\n  llrm : {b[at] if at < len(b) else '<end>'}")
    print(f"files {len(files)} total / {compared} compared / {failed} failed / {missing} missing")
    print(f"edges in opt's output {edges}")
    if not edges:
        bad.append("premise: opt's output holds no edge")
        failed += 1
    print("\n".join(bad[:30]))
    sys.exit(1 if failed or missing else 0)


main()
