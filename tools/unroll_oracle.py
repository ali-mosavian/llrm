#!/usr/bin/env python3
"""Complete unrolling against the pinned `opt -passes=loop-unroll` on the same loops.

    tools/unroll_oracle.py [FILE.ll ...]    (default: tests/fixtures/unroll)

Each file holds one loop that leaves from one conditional block. The count is
opt's own (`print<scalar-evolution>` backedge-taken count + 1). Both results go
through `instcombine,simplifycfg,adce`; their blocks and instructions per
opcode must be equal. Prints `files total / compared / failed / missing` first,
then each mismatch. Exit 1 on any failure or a missing side. A loop opt does
not unroll fails the premise: the input says nothing.
"""
import collections
import glob
import re
import subprocess
import sys
from pathlib import Path

PINNED = Path.home() / "scratch/llvm-build/bin/opt"
COMMIT = "24.0.0git"
ROOT = Path(__file__).resolve().parent.parent
LLRM = ROOT / "target/debug/llrm-mir"


def run(cmd, stdin=None):
    return subprocess.run(cmd, input=stdin, capture_output=True, text=True)


def shape(ir):
    blocks = 0
    ops = collections.Counter()
    for line in ir.splitlines():
        s = line.strip()
        if re.match(r"^[\w.$-]+:", s):
            blocks += 1
        elif s and not s.startswith((";", "define", "}", "declare", "source_filename", "target")):
            ops[re.sub(r"^%[\w.$-]+ = ", "", s).split()[0]] += 1
    return blocks, ops


def cleaned(ir):
    r = run([str(PINNED), "-passes=instcombine,simplifycfg,adce", "-S"], stdin=ir)
    return r.stdout if r.returncode == 0 else None


def main():
    version = run([str(PINNED), "--version"]).stdout
    if COMMIT not in version:
        raise SystemExit(f"unroll_oracle: {PINNED} is not LLVM {COMMIT}")
    print([l.strip() for l in version.splitlines() if "version" in l][0])
    files = sys.argv[1:] or sorted(glob.glob(str(ROOT / "tests/fixtures/unroll/*.ll")))
    compared = failed = missing = 0
    bad = []
    for f in files:
        scev = run([str(PINNED), "-passes=print<scalar-evolution>", "-disable-output", f])
        counts = re.findall(r"Loop %\S+: backedge-taken count is i\d+ (\d+)", scev.stderr)
        want = run([str(PINNED), "-passes=loop-unroll", "-S", f])
        if scev.returncode or want.returncode or len(counts) != 1:
            missing += 1
            bad.append(f"{f}: opt rc {scev.returncode}/{want.returncode}, {len(counts)} constant counts")
            continue
        count = int(counts[0]) + 1
        got = run([str(LLRM), "--unroll", str(count), f])
        if got.returncode:
            missing += 1
            bad.append(f"{f}: llrm-mir rc {got.returncode}: {got.stderr[:200]}")
            continue
        loops = run([str(PINNED), "-passes=print<loops>", "-disable-output"], stdin=want.stdout)
        if "Loop at depth" in loops.stderr:
            failed += 1
            bad.append(f"{f}: premise: opt did not unroll it completely")
            continue
        a, b = cleaned(want.stdout), cleaned(got.stdout)
        if a is None or b is None:
            missing += 1
            bad.append(f"{f}: opt cannot read {'its own' if a is None else 'llrm-mir'}'s output")
            continue
        compared += 1
        if shape(a) != shape(b):
            failed += 1
            bad.append(f"{f}: count {count}\n  opt  : {shape(a)}\n  llrm : {shape(b)}")
    print(f"files {len(files)} total / {compared} compared / {failed} failed / {missing} missing")
    print("\n".join(bad[:20]))
    sys.exit(1 if failed or missing else 0)


main()
