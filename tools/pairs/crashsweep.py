#!/usr/bin/env python3
"""Every program of the corpus through ours at each level, machine and selector: which fail to compile, and with what.

    crashsweep.py LLC WORKDIR [--jobs N]     exit 1 on a failure outside KNOWN, or a KNOWN entry that no longer fails

A compiler crash among a pin diff's `no figures` lines reads as an excluded row; this lists them all. KNOWN names each failure's owner: a new
one, or a fixed one still listed, fails the run. The pin is not run. Needs the same environment as pindiff.
"""
from __future__ import annotations

import argparse
import collections
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pin  # noqa: E402
import pindiff  # noqa: E402

LEVELS = ("O0", "O1", "O2", "O3", "Os")
MACHINES = ("m16", "m32")
EVERY = {"m16", "m32"}


@dataclass(frozen=True)
class Known:
    name: str
    isel: str
    levels: frozenset[str]
    machines: frozenset[str]
    text: str
    owner: str

    def covers(self, failure: tuple) -> bool:
        machine, level, name, isel, text = failure
        return name == self.name and isel == self.isel and level in self.levels and machine in self.machines and self.text in text


KNOWN = [
    Known("crc", "dag", frozenset({"O2", "O3"}), frozenset(EVERY), "calcspillweights.rs:267", "w13 (regalloc): `Copy from non-existing value`, 2026-10-27"),
    Known("x_ll_div", "dag", frozenset({"O1", "O2", "O3", "Os"}), frozenset(EVERY), "Freeze", "w9 (dag builder): `freeze i64` has no expand, 2026-10-27"),
    Known("x_ll_div", "old", frozenset({"O1", "O2", "O3", "Os"}), frozenset(EVERY), "freeze", "none: the old selector retires at the flip"),
    Known("lru", "-", frozenset(LEVELS), frozenset(EVERY), "clang:", "llrm-clang-frontend: `far` in a later declarator, i8086 fork parser fix, #5074"),
]


def unexpected(failures: list[tuple]) -> list[tuple]:
    return [failure for failure in failures if not any(entry.covers(failure) for entry in KNOWN)]


def fixed(failures: list[tuple]) -> list[Known]:
    """KNOWN entries none of `failures` matches: fixed, or not run."""
    return [entry for entry in KNOWN if not any(entry.covers(failure) for failure in failures)]


def one(llc: str, work: Path, src: Path, machine: str, level: str, name: str) -> list[tuple]:
    cfg = pindiff.MACHINES[machine]
    target = pin.target(machine)
    binary, flags = pindiff.clang_flags(machine)
    raw = work / f"{machine}-{level}-{name}.raw.ll"
    made = subprocess.run([binary, *flags, f"-march={target.cpu}", *pindiff.front_end_flags(level, False), "-o", str(raw), str(src / f"{name}.c")], capture_output=True, text=True)
    if made.returncode:
        return [(machine, level, name, "-", "clang: " + (made.stderr.strip().splitlines() or ["failed"])[0][:100])]
    text = raw.read_text()
    if pindiff.irguard.undefined_shifts(text):
        return [(machine, level, name, "-", "source: undefined shift")]
    text = re.sub(r"^target triple.*\n", "", text, flags=re.M)
    text = re.sub(r"^target datalayout.*$", f'target datalayout = "{pindiff.cc_entry(machine)["datalayout"]}"', text, flags=re.M)
    ours = work / f"{machine}-{level}-{name}.ll"
    ours.write_text(text)
    failures = []
    for isel in ("dag", "old"):
        ran = subprocess.run([llc, f"-m{cfg['bits']}", f"-{level}", f"-march={cfg['march']}", f"--isel={isel}", f"-mabi={cfg['abi']}", "-S", "-o", "/dev/null", str(ours)], capture_output=True, text=True, timeout=300)
        if ran.returncode:
            lines = ran.stderr.strip().splitlines() or ["failed"]
            first = next((line for line in lines if "panicked" in line), lines[0])
            after = lines[lines.index(first) + 1] if lines.index(first) + 1 < len(lines) else ""
            failures.append((machine, level, name, isel, f"{first[:140]} | {after[:80]}".strip()))
    return failures


def sweep(llc: str, work: Path, jobs: int = 8) -> tuple[int, list[tuple]]:
    work.mkdir(parents=True, exist_ok=True)
    src = pindiff.corpus_sources(work / "src")
    names = sorted(path.stem for path in src.glob("*.c"))
    todo = [(machine, level, name) for machine in MACHINES for level in LEVELS for name in names]
    with ThreadPoolExecutor(jobs) as pool:
        failures = [failure for found in pool.map(lambda job: one(llc, work, src, *job), todo) for failure in found]
    return len(todo) * 2, failures


def verdict(failures: list) -> tuple[int, list[str]]:
    """Exit code and report lines: only a crash outside KNOWN fails; a KNOWN entry that stopped crashing is reported."""
    bad = unexpected(failures)
    lines = ["UNEXPECTED " + " ".join(map(str, failure)) for failure in bad]
    lines += [f"FIXED? {e.name} {e.isel} no longer fails; remove it from KNOWN ({e.owner})" for e in fixed(failures)]
    return (1 if bad else 0), lines


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("llc")
    parser.add_argument("work")
    parser.add_argument("--jobs", type=int, default=8)
    args = parser.parse_args(argv)
    configs, failures = sweep(args.llc, Path(args.work), args.jobs)
    print(f"# crashsweep: {configs} configs, {len(failures)} failures")
    counts = collections.Counter((machine, level, isel) for machine, level, _, isel, _ in failures)
    for key, count in sorted(counts.items()):
        print(" ", *key, count)
    code, lines = verdict(failures)
    print(*lines, sep="\n")
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
