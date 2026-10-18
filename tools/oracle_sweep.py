#!/usr/bin/env python3
"""Every checking oracle over the BASIC and Nib programs: the standing coverage the C-only runs (tools/identical.py's corpus) lack.

    tools/oracle_sweep.py [--bin DIR] [--jobs N] [--levels=-O1,-O2,...] [--machines=-m16,-m32]

Compiles each bench/*/*.bas and *.nib, tests/run/{bas,nib}/*, and examples/**/*.nib at each level and machine with every `LLRM_CHECK_*`
oracle on, using `llrm-qb` and `llrm-nib` of DIR (default target/release). A refusal the compiler says in words (a library without `main`, a
convention the target does not define, a literal that does not fit) is not a failure; a panic, or a non-zero exit without a message, is. It
prints each distinct panic with its programs and exits 1 if there is one. Repeat it before a change to a pass or an analysis lands.

A sweep that finds nothing proves nothing unless the oracles ran. Before the sweep each oracle is run alone on the largest programs
and must cost the compile at least 0.05% more instructions (`perf stat`, user mode) than the same compile with a variable of the same length that no oracle reads; one that does not is named and
the sweep stops with exit 2 (`--allow-unexercised NAME` for one the target cannot reach, said aloud in the output).
"""

from __future__ import annotations

import argparse
import collections
import glob
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CHECKS = ("DATAFLOW", "FACTS", "CLOBBERED", "COALESCE", "REPLAY", "INTERN", "SKIPPED", "STALE")
# What a front end says in words and exits 1/2 for: not an oracle's finding.
REFUSED = re.compile(
    r"entry function '.*' does not exist|this target defines no|is not available: the block is|past DGROUP's|does not fit|"
    r"declare it 'huge'|unrecognized arguments"
)


def programs() -> list[tuple[str, Path]]:
    found = []
    for ext, tool in (("bas", "llrm-qb"), ("nib", "llrm-nib")):
        paths = glob.glob(f"{ROOT}/bench/*/*.{ext}") + glob.glob(f"{ROOT}/tests/run/{ext}/*.{ext}")
        paths += glob.glob(f"{ROOT}/examples/**/*.{ext}", recursive=True)
        found += [(tool, Path(path)) for path in sorted(set(paths))]
    return found


def classify(returncode: int, stderr: str) -> str | None:
    """None where the compile is fine or refused in words; else the finding's first line."""
    if returncode == 0:
        return None
    panic = re.search(r"panicked at ([^\n]*)\n([^\n]*)", stderr)
    if panic:
        return re.sub(r"\d+ sound edits", "N sound edits", panic.group(2))[:120]
    lines = [line for line in stderr.splitlines() if line.strip()]
    if lines and REFUSED.search(lines[-1]):
        return None
    return "exit %d: %s" % (returncode, lines[-1][:100] if lines else "no message")


def instructions(binaries: Path, job: tuple[str, Path, str, str], env_extra: dict[str, str]) -> int | None:
    """User-mode instructions of one compile with `env_extra` added, or None where `perf` cannot say."""
    tool, source, level, machine = job
    out = Path(os.environ.get("TMPDIR", "/tmp")) / f"probe-{os.getpid()}-{abs(hash((job, tuple(env_extra))))}.o"
    command = ["perf", "stat", "-x,", "-e", "instructions:u", str(binaries / tool), str(source), level, machine, "-o", str(out)]
    try:
        done = subprocess.run(command, capture_output=True, text=True, env={**os.environ, **env_extra})
    except FileNotFoundError:
        return None
    out.unlink(missing_ok=True)
    for line in done.stderr.splitlines():
        if "instructions" in line and line.split(",")[0].isdigit():
            return int(line.split(",")[0])
    return None


def exercised(binaries: Path, probes: list[tuple[str, Path, str, str]]) -> dict[str, tuple[int, int] | None]:
    """For each oracle: (instructions with a variable of the same length that no oracle reads, with the oracle) over `probes`; None
    where perf cannot say. The control carries the cost of the variable's presence, which a compiler ignoring the oracle also pays."""
    found: dict[str, tuple[int, int] | None] = {}
    for name in CHECKS:
        control = [instructions(binaries, job, {"LLRM_CHECK_" + "X" * len(name): "1"}) for job in probes]
        with_it = [instructions(binaries, job, {f"LLRM_CHECK_{name}": "1"}) for job in probes]
        found[name] = None if None in control or None in with_it else (sum(control), sum(with_it))
    return found


def compile_one(job: tuple[str, Path, str, str], binaries: Path) -> tuple[tuple[str, Path, str, str], str | None]:
    tool, source, level, machine = job
    out = Path(os.environ.get("TMPDIR", "/tmp")) / f"sweep-{os.getpid()}-{abs(hash(job))}.o"
    env = {**os.environ, **{f"LLRM_CHECK_{name}": "1" for name in CHECKS}}
    done = subprocess.run([str(binaries / tool), str(source), level, machine, "-o", str(out)], capture_output=True, text=True, env=env)
    out.unlink(missing_ok=True)
    return job, classify(done.returncode, done.stderr)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--bin", type=Path, default=ROOT / "target/release")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "8")))
    parser.add_argument("--levels", default="-O1,-O2,-O3,-Os,-Omax")
    parser.add_argument("--machines", default="-m16,-m32")
    parser.add_argument("--allow-unexercised", action="append", default=[], metavar="NAME", help="an oracle the sweep may skip")
    args = parser.parse_args()
    found = programs()
    biggest = sorted(found, key=lambda one: -one[1].stat().st_size)[:3]
    ran = exercised(args.bin, [(t, p, "-O2", "-m32") for t, p in biggest])
    unexercised = []
    for name, counts in ran.items():
        if counts is None:
            print(f"oracle {name}: not measurable (no perf, or a probe did not compile)")
        else:
            print(f"oracle {name}: {counts[0]} -> {counts[1]} instructions on the {len(biggest)} largest programs")
        if (counts is None or counts[1] - counts[0] < max(1000, counts[0] // 2000)) and name not in args.allow_unexercised:
            unexercised.append(name)
    if unexercised:
        print("never exercised: " + ", ".join(unexercised) + " (a sweep where an oracle did not run proves nothing about it)")
        return 2
    jobs = [(t, p, l, m) for t, p in found for l in args.levels.split(",") for m in args.machines.split(",")]
    with ThreadPoolExecutor(args.jobs) as pool:
        results = list(pool.map(lambda job: compile_one(job, args.bin), jobs))
    findings: dict[str, set[str]] = collections.defaultdict(set)
    for (tool, source, level, machine), why in results:
        if why:
            findings[why].add(f"{source.name} {level} {machine}")
    print(f"{len(results)} compiles, {sum(map(len, findings.values()))} findings in {len(findings)} kinds")
    for why, where in sorted(findings.items(), key=lambda item: -len(item[1])):
        print(f"{len(where):4}  {why}\n      e.g. {sorted(where)[:3]}")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
