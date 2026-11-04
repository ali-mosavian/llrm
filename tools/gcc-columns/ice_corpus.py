#!/usr/bin/env python3
"""Corpus ICE gate for a cc1 swap: compile every file of the corpus at O1/O2/Os with two cc1 and list what ICEs (or times out) on the new one only.

    ice_corpus.py --old CC1 --new CC1 [--fork-tests DIR] [--jobs N]

Corpus: compiler-rt lib/ and config/ C files, the benches' kernels (-DK_<name>) and the fork's spike-tests/*.c.
Exit 1 if the new cc1 loses a file the old one compiles. Files that fail the same way on both are listed, not counted.
"""
import argparse
import concurrent.futures as cf
import glob
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
RT = os.path.join(ROOT, "runtime", "compiler-rt")
LEVELS = ("O1", "O2", "Os")


def classify(returncode, text, timed_out=False):
    """ok | ice | timeout | error: an ICE is gcc's own message, an error is the source's (missing header, unsupported type)."""
    if timed_out:
        return "timeout"
    if "internal compiler error" in text or "unable to find a register to spill" in text:
        return "ice"
    return "ok" if returncode == 0 else "error"


def compile_one(cc1, src, level, defs):
    cmd = [cc1, "-quiet", f"-{level}", "-I" + os.path.join(HERE, "sf-z80", "inc"), "-I" + os.path.join(RT, "include"),
           "-DSF_INTABI", *defs, src, "-o", "/dev/null"]
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=300, preexec_fn=lambda: __import__("resource").setrlimit(
            __import__("resource").RLIMIT_AS, (16 * 10**9, 16 * 10**9)))
    except subprocess.TimeoutExpired:
        return classify(1, "", True)
    return classify(p.returncode, p.stdout + p.stderr)


def corpus(fork_tests):
    items = []
    for f in sorted(glob.glob(os.path.join(RT, "lib", "**", "*.c"), recursive=True) + glob.glob(os.path.join(RT, "config", "**", "*.c"), recursive=True)):
        items.append((os.path.relpath(f, ROOT), f, []))
    sys.path.insert(0, HERE)
    import gcc_z80_bench as Z
    kernels = os.path.join(ROOT, "crates", "target", "llrm-6502", "tests", "bench", "kernels.c")
    for k in Z.K:
        items.append((f"kernels.c:{k[0]}", kernels, [f"-DK_{k[0]}"]))
    for f in sorted(glob.glob(os.path.join(fork_tests, "*.c"))):
        items.append((os.path.join("spike-tests", os.path.basename(f)), f, []))
    return items


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--old", required=True)
    ap.add_argument("--new", required=True)
    ap.add_argument("--fork-tests", default=os.path.expanduser("~/scratch/gcc-spike/gcc/gcc/config/z80/spike-tests"))
    ap.add_argument("--jobs", type=int, default=4)
    a = ap.parse_args()
    items = corpus(a.fork_tests)
    jobs = [(name, src, defs, lvl, which) for name, src, defs in items for lvl in LEVELS for which in ("old", "new")]
    res = {}
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = {ex.submit(compile_one, getattr(a, w), s, l, d): (n, l, w) for n, s, d, l, w in jobs}
        for f in cf.as_completed(futs):
            res[futs[f]] = f.result()
    lost, same_bad = [], []
    for name, _, _ in items:
        for lvl in LEVELS:
            o, n = res[(name, lvl, "old")], res[(name, lvl, "new")]
            if o == "ok" and n != "ok":
                lost.append((name, lvl, n))
            elif o != "ok" and n != "ok":
                same_bad.append((name, lvl, o, n))
    cells = len(items) * len(LEVELS)
    print(f"{len(items)} files x {len(LEVELS)} levels = {cells} compiles per cc1; old ok {sum(1 for k, v in res.items() if k[2] == 'old' and v == 'ok')}, new ok {sum(1 for k, v in res.items() if k[2] == 'new' and v == 'ok')}")
    print(f"new-only failures: {len(lost)}")
    for x in lost:
        print("  LOST", *x)
    print(f"failing on both: {len(same_bad)}")
    for x in same_bad:
        print("  both", *x)
    return 1 if lost else 0


if __name__ == "__main__":
    sys.exit(main())
