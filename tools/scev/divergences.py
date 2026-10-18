#!/usr/bin/env python3
"""Writes tools/scev/divergences.txt: the expressions on which the pinned
LLVM's answer is wrong, its answer and ours.

    tools/scev/divergences.py

`EXPRESSION TAB LLVM TAB OURS`, LLVM's column `SEGFAULT` when the oracle
dies. All reduce to one cause (DEVIATE 8): `getAddExpr` and `getMulExpr`
find the recurrence they just rewrote with `Ops[i] == AddRec`, which
compares the use bits too, so a recurrence held with a use flag is never
found; the search runs past the array (a segfault) or, at `-1 * (udiv *
addrec)`, leaves the scale out. `opt -passes='print<scalar-evolution>'` on
the same IR (no use flags there) gives ours.
"""
import subprocess, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fuzz

AR = "(use nuw (addrec -:- 1 (u 2) (u 1)))"
CASES = [
    f"(urem (c 16 100) {AR})",
    f"(urem (u 2) {AR})",
    f"(mul -:- (c 16 7) (udiv (u 2) {AR}) {AR})",
    f"(mul -:- (u 0) (udiv (u 2) {AR}) {AR})",
    f"(add -:- (u 0) (udiv (u 2) {AR}) {AR})",
    f"(add -:- (c 16 5) (udiv (u 2) {AR}) {AR})",
]

rows = []
for line in CASES:
    want = subprocess.run([str(fuzz.ORACLE), str(fuzz.FIXTURE)], input=line + "\n", capture_output=True, text=True)
    got = subprocess.run([str(fuzz.LLRM), "--scev", str(fuzz.FIXTURE)], input=line + "\n", capture_output=True, text=True)
    llvm = "SEGFAULT" if want.returncode in (-11, 139) else want.stdout.strip()
    rows.append(f"{line}\t{llvm}\t{got.stdout.strip()}")
(fuzz.HERE / "divergences.txt").write_text("\n".join(rows) + "\n")
print(f"{len(rows)} cases written")
