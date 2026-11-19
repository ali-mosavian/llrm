#!/usr/bin/env python3
"""The pindiff rows that grew between two parity commits, per machine and level.

    drift.py SHA_A SHA_B [--configs m32:O2,m16:Os,...] [--pairs DIR] [--work DIR] [--jobs N]   build (or find) both dists, run pindiff on each, list the growth
    drift.py --tables BEFORE.txt AFTER.txt                                                      the same listing for two tables already made

A row grew when its instructions or its padding-free bytes did; a row that shrank or held is not listed. Exit 1 when any row grew, so a landing wave can be gated on it.
Each commit's llrm-llc is `PAIRS/sbp-<sha>/dist/llrm-llc` (made by `pair_build.sh` when absent); pindiff is this checkout's, so both sides run one corpus.
"""
from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import rowdelta  # noqa: E402

CONFIGS = ["m32:O1", "m32:O2", "m32:Os", "m16:O1", "m16:O2", "m16:Os"]


def grown(before: str, after: str) -> list[tuple[str, int, int, str]]:
    """`(name, instructions grown, bytes grown, text)` of each row that grew, the most instructions first."""
    old, new = rowdelta.parse(before), rowdelta.parse(after)
    found = []
    for name in new:
        if name not in old:
            continue
        (ins0, _, free0, _), (ins1, pin, free1, pin_free) = old[name], new[name]
        if ins1 > ins0 or free1 > free0:
            text = f"{name}: ins {ins0}->{ins1} ({ins1 - ins0:+d}, pin {pin}), nopad {free0}->{free1} ({free1 - free0:+d}, pin {pin_free})"
            found.append((name, ins1 - ins0, free1 - free0, text))
    return sorted(found, key=lambda row: (-row[1], -row[2], row[0]))


def header(shas: list[str], llcs: list[Path]) -> list[str]:
    """What produced the numbers: the pindiff path is llrm-llc on clang's un-optimised IR, not the product's llrm-cc, and a row from it is not the product's."""
    lines = ["# instrument: llrm-llc via pindiff (clang IR, no passes; ours = llrm-llc's own pipeline, pin = its opt + llc), NOT llrm-cc: a row here is not the product path's"]
    for side, (sha, llc) in zip(("before", "after"), zip(shas, llcs)):
        lines.append(f"# {side}: {sha} llrm-llc md5 {hashlib.md5(llc.read_bytes()).hexdigest()[:12]}")
    return lines


def llc_of(sha: str, pairs: Path) -> Path:
    full = subprocess.run(["git", "rev-parse", "--verify", f"{sha}^{{commit}}"], cwd=HERE, capture_output=True, text=True, check=True).stdout.strip()
    llc = pairs / f"sbp-{full}" / "dist" / "llrm-llc"
    if not llc.exists():
        subprocess.run(["bash", str(HERE / "pair_build.sh"), str(pairs), full], cwd=HERE, check=True)
    return llc


def table_of(llc: Path, config: str, work: Path) -> str:
    machine, level = config.split(":")
    done = subprocess.run(
        [sys.executable, str(HERE / "pindiff.py"), machine, level, "--llc", str(llc), "--work", str(work / f"{llc.parents[1].name}-{machine}-{level}")],
        capture_output=True,
        text=True,
    )
    return done.stdout


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("shas", nargs="*")
    parser.add_argument("--tables", nargs=2, metavar=("BEFORE", "AFTER"))
    parser.add_argument("--configs", default=",".join(CONFIGS))
    parser.add_argument("--pairs", type=Path, default=Path.home() / "scratch/pairs")
    parser.add_argument("--work", type=Path, default=Path.home() / "scratch/drift")
    parser.add_argument("--jobs", type=int, default=2)
    args = parser.parse_args(argv)
    if args.tables:
        rows = grown(Path(args.tables[0]).read_text(), Path(args.tables[1]).read_text())
        print("\n".join(text for *_, text in rows))
        return 1 if rows else 0
    if len(args.shas) != 2:
        parser.error("two commits, or --tables BEFORE AFTER")
    args.work.mkdir(parents=True, exist_ok=True)
    llcs = [llc_of(sha, args.pairs) for sha in args.shas]
    configs = args.configs.split(",")
    with ThreadPoolExecutor(args.jobs) as pool:
        tables = list(pool.map(lambda job: table_of(llcs[job[0]], job[1], args.work), [(side, config) for config in configs for side in (0, 1)]))
    print("\n".join(header(args.shas, llcs)))
    any_grown = False
    for number, config in enumerate(configs):
        before, after = tables[2 * number], tables[2 * number + 1]
        if not rowdelta.parse(before) or not rowdelta.parse(after):
            print(f"{config}: no rows from {'the first' if not rowdelta.parse(before) else 'the second'} commit's pindiff")
            any_grown = True
            continue
        for *_, text in grown(before, after):
            print(f"{config.replace(':', ' ')} {text}")
            any_grown = True
    return 1 if any_grown else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
