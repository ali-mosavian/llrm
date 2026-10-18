#!/usr/bin/env python3
"""One-off fuzzing of llrm's SCEV folds against the pinned LLVM's.

    tools/scev/fuzz.py [COUNT] [SEED]

Generates COUNT random valid expressions (the language at the top of
oracle.cpp), builds each with the pinned LLVM (tools/scev/build.sh) and with
`llrm-mir --scev`, and prints the lines where the two texts differ. Not part
of the test suite: the suite holds fixed golden cases (tools/scev/golden.txt).
"""
import random, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
ORACLE = Path.home() / "scratch/scev-oracle/oracle"
LLRM = ROOT / "target/debug/llrm-mir"
FIXTURE = HERE / "fixture.ll"
# argument number by width: the fixture's signature
ARGS = {16: [0, 1, 2], 32: [3, 4], 64: [5], 8: [6], 1: [7]}
WIDTHS = [8, 16, 32, 64]
INTERESTING = [0, 1, 2, 3, 4, 5, 7, 8, 16, 255, 256, 1000]


def const(rng, w):
    v = rng.choice(INTERESTING + [2 ** (w - 1), 2 ** w - 1, 2 ** w - 2, rng.randrange(2 ** w)])
    return f"(c {w} {v % 2 ** w})"


def flags(rng):
    parts = [f for f in ("nuw", "nsw") if rng.random() < 0.3]
    return ",".join(parts) or "-"


def leaf(rng, w):
    if rng.random() < 0.4 or w not in ARGS:
        return const(rng, w)
    return f"(u {rng.choice(ARGS[w])})"


def invariant(rng, w, depth):
    """An expression with no recurrence."""
    if depth == 0 or rng.random() < 0.3:
        return leaf(rng, w)
    return gen(rng, w, depth, recurrences=False)


def gen(rng, w, depth, recurrences=True):
    if depth == 0:
        return leaf(rng, w)
    kind = rng.choice(
        ["add", "mul", "udiv", "urem", "minus", "neg", "not", "minmax", "cast", "use", "leaf", "addrec", "seq"]
    )
    sub = lambda ww=w: gen(rng, ww, depth - 1, recurrences)
    if kind == "leaf":
        return leaf(rng, w)
    if kind in ("add", "mul"):
        ops = " ".join(sub() for _ in range(rng.choice([2, 2, 3, 4])))
        return f"({kind} {flags(rng)}:- {ops})"
    if kind in ("udiv", "urem", "minus"):
        return f"({kind} {sub()} {sub()})"
    if kind in ("neg", "not"):
        return f"({kind} {sub()})"
    if kind == "minmax":
        op = rng.choice(["smax", "umax", "smin", "umin"])
        return f"({op} {' '.join(sub() for _ in range(rng.choice([2, 2, 3])))})"
    if kind == "seq":
        return f"(sequmin {sub()} {sub()})"
    if kind == "use":
        return f"(use {flags(rng)} {sub()})"
    if kind == "cast":
        others = [x for x in WIDTHS if x != w]
        other = rng.choice(others)
        if other > w:
            op = rng.choice(["trunc"])  # source wider: trunc down to w
            return f"(trunc {w} {gen(rng, other, depth - 1, recurrences)})"
        op = rng.choice(["zext", "sext", "anyext"])
        return f"({op} {w} {gen(rng, other, depth - 1, recurrences)})"
    if kind == "addrec" and recurrences:
        loop = rng.choice([0, 1, 2])
        ops = [invariant(rng, w, depth - 1) for _ in range(rng.choice([2, 2, 3]))]
        if loop == 1 and rng.random() < 0.3:
            ops[0] = f"(addrec {flags(rng)}:- 0 {invariant(rng, w, 1)} {invariant(rng, w, 1)})"
        return f"(addrec {flags(rng)}:- {loop} {' '.join(ops)})"
    return leaf(rng, w)


def run(tool, lines):
    done = subprocess.run(tool, input="\n".join(lines) + "\n", capture_output=True, text=True)
    return done


def main():
    count = int(sys.argv[1]) if len(sys.argv) > 1 else 2000
    seed = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    rng = random.Random(seed)
    lines = [gen(rng, rng.choice(WIDTHS), rng.choice([1, 2, 3, 4])) for _ in range(count)]
    differ = crashed = same = 0
    shown = 0
    for at in range(0, len(lines), 1):
        pass
    # One process per expression: an LLVM assertion kills only its own.
    for line in lines:
        want = subprocess.run([str(ORACLE), str(FIXTURE)], input=line + "\n", capture_output=True, text=True)
        got = subprocess.run([str(LLRM), "--scev", str(FIXTURE)], input=line + "\n", capture_output=True, text=True)
        if want.returncode != 0:
            continue  # not a valid call for LLVM
        if got.returncode != 0:
            crashed += 1
            if shown < 15:
                shown += 1
                print(f"CRASH {line}\n  llvm: {want.stdout.strip()}\n  llrm: {got.stderr.strip().splitlines()[-1:]}")
            continue
        if want.stdout != got.stdout:
            differ += 1
            if shown < 15:
                shown += 1
                print(f"DIFF {line}\n  llvm: {want.stdout.strip()}\n  llrm: {got.stdout.strip()}")
        else:
            same += 1
    print(f"expressions {len(lines)}: same {same} / differ {differ} / crashed {crashed}")


if __name__ == "__main__":
    main()
