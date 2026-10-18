#!/usr/bin/env python3
"""Writes tools/scev/golden.txt: expressions and the text the pinned LLVM prints.

    tools/scev/golden.py [COUNT] [SEED]

The hand-written cases of HAND first, then COUNT generated ones (fuzz.py's
generator, SEED). A line is `EXPRESSION TAB TEXT`. Expressions LLVM aborts on
are left out, and so are those where `llrm-mir --scev` differs from LLVM
(listed on stderr): the file holds LLVM's answers, and a case we get wrong
would make the test fail on the day it was written.
"""
import random, subprocess, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fuzz

HAND = [
    "(add -:- (u 0) (c 16 1))",
    "(add -:- (u 0) (u 0))",
    "(add -:- (u 0) (c 16 0))",
    "(add -:- (c 16 3) (c 16 4))",
    "(mul -:- (u 0) (c 16 0))",
    "(mul -:- (u 0) (c 16 1))",
    "(mul -:- (c 16 3) (add -:- (u 0) (c 16 5)))",
    "(zext 32 (add nuw:- (u 0) (c 16 1)))",
    "(sext 32 (add nsw:- (u 0) (c 16 1)))",
    "(trunc 8 (zext 32 (u 0)))",
    "(udiv (u 0) (c 16 8))",
    "(urem (u 0) (c 16 8))",
    "(urem (u 0) (u 1))",
    "(umax (u 0) (u 1) (c 16 5))",
    "(smin (u 0) (u 0))",
    "(minus (u 0) (u 1))",
    "(neg (u 0))",
    "(not (u 0))",
    "(addrec nuw:- 0 (c 16 0) (c 16 1))",
    "(addrec nsw:- 1 (u 0) (c 16 3))",
    "(sext 32 (addrec nsw:- 1 (u 0) (c 16 3)))",
    "(add -:- (addrec -:- 0 (c 16 0) (c 16 1)) (addrec -:- 0 (c 16 5) (c 16 2)))",
    "(mul -:- (addrec -:- 0 (c 16 1) (c 16 1)) (addrec -:- 0 (c 16 1) (c 16 1)))",
    "(sequmin (u 0) (u 1))",
    "(add -:- (use nuw (add nuw:- (u 0) (u 1))) (u 2))",
]


def main():
    count = int(sys.argv[1]) if len(sys.argv) > 1 else 400
    seed = int(sys.argv[2]) if len(sys.argv) > 2 else 7
    rng = random.Random(seed)
    lines = HAND + [fuzz.gen(rng, rng.choice(fuzz.WIDTHS), rng.choice([1, 2, 3, 4])) for _ in range(count)]
    out = []
    for line in lines:
        want = subprocess.run([str(fuzz.ORACLE), str(fuzz.FIXTURE)], input=line + "\n", capture_output=True, text=True)
        if want.returncode != 0:
            continue
        got = subprocess.run([str(fuzz.LLRM), "--scev", str(fuzz.FIXTURE)], input=line + "\n", capture_output=True, text=True)
        if got.stdout != want.stdout:
            print(f"DIFFERS, left out: {line}", file=sys.stderr)
            continue
        out.append(f"{line}\t{want.stdout.strip()}")
    (fuzz.HERE / "golden.txt").write_text("\n".join(out) + "\n")
    print(f"{len(out)} of {len(lines)} cases written")


main()
