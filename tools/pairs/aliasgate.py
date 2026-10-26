#!/usr/bin/env python3
"""The never-a-NoAlias-gained gate for a change to the alias stack's inputs.

    tools/pairs/aliasgate.py --dumper BASE_LLRM_C --old OLD_ALIAS_PAIRS --new NEW_ALIAS_PAIRS [--src DIR] [--jobs N]

The dumper (one llrm-c, so both sides read one IR) writes each source's per-pass
module dumps (`--dump`); at three stages of each (the first, the last of the
walk, the last) the OLD and NEW builds of `alias_pairs` record the stack's
answers for every load and store pair and every call against every access
(`llrm_analysis::aliasdiff`), and `alias_pairs --diff` compares them. Exit 1 on
a gained proof or an unmatched row; lost proofs are counted. DIR holds the
wrapped sources vsgcc compiles (`ROWS_WORK/<name>/b/*.c`).
"""
from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

FLAGS = {"m32": ["-m32", "-mabi=sysv", "-march=i486"], "m16": ["-m16", "-march=i386"]}


def stages(directory: Path) -> list[Path]:
    """The dumps to compare: the first, the last of the walk, the last."""
    files = sorted(directory.glob("*.ll"))
    walk = [one for one in files if "walk" in one.name or "interprocedural" in one.name]
    picked = [files[0]] if files else []
    picked += walk[-1:] + files[-1:]
    return list(dict.fromkeys(picked))


def one(args: argparse.Namespace, source: Path, target: str, work: Path) -> tuple[str, int, int, int, int, list[str]]:
    name = f"{source.stem}-{target}"
    dump = work / name
    done = subprocess.run([args.dumper, *FLAGS[target], "-O2", "--dump", str(dump), "-o", str(work / f"{name}.o"), str(source)], capture_output=True, text=True)
    if done.returncode != 0:
        return name, 0, 0, 0, 0, [f"{name}: the dumper failed: {(done.stderr.strip().splitlines() or ['?'])[-1][:100]}"]
    totals = [0, 0, 0, 0]
    failures: list[str] = []
    for stage in stages(dump):
        recorded = []
        for label, tool in (("old", args.old), ("new", args.new)):
            out = work / f"{name}-{stage.stem}.{label}"
            with out.open("w") as handle:
                made = subprocess.run([tool, str(stage)], stdout=handle, stderr=subprocess.PIPE, text=True)
            if made.returncode != 0:
                failures.append(f"{name} {stage.name}: {label} failed: {(made.stderr.strip().splitlines() or ['?'])[-1][:100]}")
            recorded.append(out)
        compared = subprocess.run([args.new, "--diff", str(recorded[0]), str(recorded[1])], capture_output=True, text=True)
        head = compared.stdout.splitlines()[0].split() if compared.stdout else []
        if len(head) >= 10:
            for at, key in enumerate((1, 3, 5, 7)):
                totals[at] += int(head[key])
        failures += [f"{name} {stage.name}: {line}" for line in compared.stdout.splitlines()[1:] if line.startswith("FAIL")]
    return (name, *totals, failures)  # type: ignore[return-value]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dumper", required=True)
    parser.add_argument("--old", required=True)
    parser.add_argument("--new", required=True)
    parser.add_argument("--src", default=str(Path.home() / "scratch/rows/main24d/b"))
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--targets", default="m32,m16")
    args = parser.parse_args()
    sources = sorted(Path(args.src).glob("*.c"))
    with tempfile.TemporaryDirectory(dir=str(Path.home() / "scratch")) as made:
        work = Path(made)
        with ThreadPoolExecutor(args.jobs) as pool:
            results = list(pool.map(lambda pair: one(args, pair[0], pair[1], work), [(s, t) for s in sources for t in args.targets.split(",")]))
    same = sum(r[1] for r in results)
    lost = sum(r[2] for r in results)
    changed = sum(r[3] for r in results)
    gained = sum(r[4] for r in results)
    failures = [line for r in results for line in r[5]]
    print(f"aliasgate: {len(results)} programs: same {same} lost {lost} changed {changed} gained {gained}; {len(failures)} failures")
    for line in failures[:60]:
        print("  FAIL", line)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
