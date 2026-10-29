#!/usr/bin/env python3
"""Per function, the instruction mnemonics the DAG path selects against the old selector's.

    tools/dagisel/isel_compare.py LLRM_CC [--m16] [--src DIR] [--jobs N] [--out FILE]

Each program is compiled twice with `ISEL_DUMP=1` (`--isel=old`, and `--isel=dag`, which refuses what it
cannot take); the first `=== ISel` listing of each is read as a multiset of mnemonics (`mov`s left out: copies). A function lands in one
bucket: `refused: WHY` (the DAG path took none), `equal`, or `differs: <mnemonic> old N dag M` for the mnemonic
whose count differs most. The total is every function the old selector selected.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import llrmbin  # noqa: E402

M32 = ["-c", *llrmbin.c_args(32), "-mabi=sysv", "-O2"]
M16 = ["-c", *llrmbin.c_args(16), "-O2"]


def listings(stderr: str) -> dict[str, Counter]:
    """{function: mnemonic counts} from the first `=== ISel` listing of each function."""
    out: dict[str, Counter] = {}
    in_isel = False
    current = None
    for line in stderr.splitlines():
        if line.startswith("=== "):
            in_isel = line.startswith("=== ISel")
            current = None
            continue
        if not in_isel:
            continue
        head = re.match(r"  (\S+): \d+ instructions", line)
        if head:
            current = head[1]
            out.setdefault(current, Counter())
            continue
        body = re.match(r"\s+0x[0-9a-f]+\s+(.*?)(\s*;.*)?$", line)
        if current and body and body[1]:
            text = body[1]
            if " := " in text:
                text = text.split(" := ", 1)[1]
            mnemonic = text.split(" ")[0]
            # A register copy is not selection: the old selector returns a value through `requires`, the DAG path copies it.
            if mnemonic != "mov":
                out[current][mnemonic] += 1
    return out


def one(llrm_cc: str, flags: list[str], source: Path) -> list[tuple[str, str, str]]:
    env = {**os.environ, **llrmbin.cc_environment(), "ISEL_DUMP": "1", "LLRM_DEBUG": "isel"}
    run = lambda mode: subprocess.run([llrm_cc, *flags, f"--isel={mode}", "-S", "-o", "/dev/null", str(source)], capture_output=True, text=True, env=env, timeout=600)
    old, dag = run("old"), run("shadow")
    want, have = listings(old.stdout), listings(dag.stdout)
    refused = {m[1]: m[2] for m in re.finditer(r"\[isel\] (\S+) refused: (.*)", dag.stderr)}
    rows = []
    for name, counts in want.items():
        if name in refused:
            bucket = "refused: " + re.sub(r'"[^"]*"', "", refused[name])[:60]
        elif name not in have:
            bucket = "no DAG listing"
        elif have[name] == counts:
            bucket = "equal"
        else:
            delta = Counter({k: abs(counts[k] - have[name][k]) for k in set(counts) | set(have[name])})
            top = max(delta.items(), key=lambda kv: kv[1])[0]
            bucket = f"differs: {top} old {counts[top]} dag {have[name][top]}"
        rows.append((source.stem, name, bucket))
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("llrm_cc")
    parser.add_argument("--m16", action="store_true")
    parser.add_argument("--src", default=str(Path.home() / "scratch/rows/base/b"))
    parser.add_argument("--jobs", type=int, default=6)
    parser.add_argument("--out", default="")
    args = parser.parse_args()
    flags = M16 if args.m16 else M32
    sources = sorted(Path(args.src).glob("*.c"))
    with ThreadPoolExecutor(args.jobs) as pool:
        rows = [r for part in pool.map(lambda s: one(args.llrm_cc, flags, s), sources) for r in part]
    buckets = Counter(b if not b.startswith("differs") else "differs" for _, _, b in rows)
    print(f"{buckets['equal']} equal of {len(rows)} functions in {len(sources)} programs")
    for bucket, n in buckets.most_common():
        print(f"{n:6d}  {bucket}")
    if args.out:
        Path(args.out).write_text("".join(f"{p}\t{f}\t{b}\n" for p, f, b in rows))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
