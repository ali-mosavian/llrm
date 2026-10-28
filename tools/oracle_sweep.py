#!/usr/bin/env python3
"""Every checking oracle over the BASIC and Nib programs: the standing coverage the C-only runs (tools/identical.py's corpus) lack.

    tools/oracle_sweep.py [--bin DIR] [--jobs N] [--levels=-O1,-O2,...] [--machines=-m16,-m32]

Compiles each bench/*/*.bas and *.nib, tests/run/{bas,nib}/*, and examples/**/*.nib at each level and machine with every `LLRM_CHECK_*`
oracle on, using `llrm-qb` and `llrm-nib` of DIR (default target/release). A refusal the compiler says in words (a library without `main`, a
convention the target does not define, a literal that does not fit) is not a failure; a panic, or a non-zero exit without a message, is. It
prints each distinct panic with its programs and exits 1 if there is one. Repeat it before a change to a pass or an analysis lands.

A sweep that finds nothing proves nothing unless the oracles ran. Before the sweep each oracle is run alone on the largest programs
and must have its hook reached (`LLRM_DEBUG=checks`) and cost the compile more instructions (`perf stat`, user mode) than the same compile with a variable of the same length that no oracle reads, at m16 and m32; one that does not is named and
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
import time
import tomllib
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402

# COALESCE: its hook went with the old coalescer (902b535761). SKIPPED: no corpus program skips a pass in a fixed-point round (846 compiles at
# -O1/-O3/-Os, -O2/-Omax on bench and examples, m16 and m32, reached its hook in none), so it never runs here.
CHECKS = ("DATAFLOW", "FACTS", "CLOBBERED", "REPLAY", "INTERN", "STALE")
PROBE_MACHINES = ("-m16", "-m32")  # DATAFLOW and CLOBBERED hooks are reached at m16 only
NOTABLE = 0.0005  # the old bar, a fraction of the control's instructions: printed, no longer a verdict
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


def instructions(binaries: Path, job: tuple[str, Path, str, str], env_extra: dict[str, str]) -> tuple[int | None, dict[str, int]]:
    """(user-mode instructions of one compile with `env_extra` added, or None where `perf` cannot say; the oracles its hooks reached)."""
    tool, source, level, machine = job
    out = Path(os.environ.get("TMPDIR", "/tmp")) / f"probe-{os.getpid()}-{abs(hash((job, tuple(env_extra))))}.o"
    command = ["perf", "stat", "-x,", "-e", "instructions:u", str(binaries / tool), str(source), level, machine, "-o", str(out)]
    env = {**os.environ, "LLRM_DEBUG": ",".join(filter(None, [os.environ.get("LLRM_DEBUG"), "checks"])), **env_extra}
    try:
        done = subprocess.run(command, capture_output=True, text=True, env=env)
    except FileNotFoundError:
        return None, {}
    out.unlink(missing_ok=True)
    reached = {name: int(sites) for name, sites in REACHED.findall(done.stderr)}
    for line in done.stderr.splitlines():
        if "instructions" in line and line.split(",")[0].isdigit():
            return int(line.split(",")[0]), reached
    return None, reached


def exercised(binaries: Path, probes: list[tuple[str, Path, str, str]]) -> dict[str, tuple[int, int, int] | None]:
    """For each oracle: (instructions with a variable of the same length that no oracle reads, with the oracle, call sites of its hook reached)
    over `probes`; None where perf cannot say. The control carries the cost of the variable's presence, which a compiler ignoring the oracle also pays."""
    found: dict[str, tuple[int, int, int] | None] = {}
    for name in CHECKS:
        control = [instructions(binaries, job, {"LLRM_CHECK_" + "X" * len(name): "1"})[0] for job in probes]
        with_it = [instructions(binaries, job, {f"LLRM_CHECK_{name}": "1"}) for job in probes]
        if None in control or any(count is None for count, _ in with_it):
            found[name] = None
        else:
            found[name] = (sum(control), sum(count for count, _ in with_it), sum(reached.get(name, 0) for _, reached in with_it))
    return found


REACHED = re.compile(r"^\[checks\] (\w+) (\d+)$", re.M)


def compile_one(job: tuple[str, Path, str, str], binaries: Path) -> tuple[tuple[str, Path, str, str], str | None, dict[str, int]]:
    """(the job, its finding or None, the oracles its compile reached and the call sites of each: `LLRM_DEBUG=checks`)."""
    tool, source, level, machine = job
    out = Path(os.environ.get("TMPDIR", "/tmp")) / f"sweep-{os.getpid()}-{abs(hash(job))}.o"
    debug = ",".join(filter(None, [os.environ.get("LLRM_DEBUG"), "checks"]))
    env = {**os.environ, **{f"LLRM_CHECK_{name}": "1" for name in CHECKS}, "LLRM_DEBUG": debug}
    done = subprocess.run([str(binaries / tool), str(source), level, machine, "-o", str(out)], capture_output=True, text=True, env=env)
    out.unlink(missing_ok=True)
    reached = {name: int(sites) for name, sites in REACHED.findall(done.stderr)}
    return job, classify(done.returncode, REACHED.sub("", done.stderr)), reached


def unreached(results: list[tuple], allowed: list[str]) -> list[str]:
    """The oracles no compile of the run reached (no call site asked them while on): a run that finds nothing proves nothing about them."""
    reached = collections.Counter()
    for _, _, sites in results:
        reached.update(sites)
    return [name for name in CHECKS if reached[name] == 0 and name not in allowed]


def judge(findings: dict[str, set[str]], expected: list[dict]) -> tuple[list[str], list[str], list[str]]:
    """(new findings, expected findings seen, expected entries not seen). An entry of the expected-failures file names the finding it covers
    (`match`, a substring of the finding's first line), the pull request or issue that fixes it (`fix`) and its owner. A finding no entry covers
    fails the sweep; an entry that matches nothing is paid, and fails it too: the file only shrinks."""
    new, seen, matched = [], [], set()
    for why in findings:
        hit = next((i for i, one in enumerate(expected) if one["match"] in why), None)
        if hit is None:
            new.append(why)
        else:
            matched.add(hit)
            seen.append(f"{why} (expected: {expected[hit]['fix']}, {expected[hit]['owner']})")
    unseen = [f"{one['match']} ({one['fix']}, {one['owner']})" for i, one in enumerate(expected) if i not in matched]
    return new, seen, unseen


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--bin", type=Path, default=None, help="the llrm-qb and llrm-nib directory (default: LLRM_BIN, else the target dir's release)")
    parser.add_argument("--expected", type=Path, default=None, help="the expected failures (tools/oracle_expected.toml): findings that may stay until their fix lands")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "8")))
    parser.add_argument("--levels", default="-O1,-O2,-O3,-Os,-Omax")
    parser.add_argument("--machines", default="-m16,-m32")
    parser.add_argument("--shard", metavar="I/N", help="only slice I of N of the programs (the gate's oracle-I job): no entry is judged paid by a slice")
    parser.add_argument("--no-reach", action="store_true", help="skip the check that every oracle's hook was reached (LLRM_DEBUG=checks), which needs no perf")
    parser.add_argument("--no-premise", action="store_true", help="skip the check that each oracle ran (it needs perf; the gate's oracle-premise step does it once)")
    parser.add_argument("--premise-only", action="store_true", help="only the check that each oracle ran")
    parser.add_argument("--allow-unexercised", action="append", default=[], metavar="NAME", help="an oracle the sweep may skip")
    args = parser.parse_args()
    args.bin = args.bin or llrmbin.bin_dir()
    expected = tomllib.loads(args.expected.read_text()).get("expected", []) if args.expected else []
    started = time.perf_counter()
    found = programs()
    biggest = sorted(found, key=lambda one: -one[1].stat().st_size)[:3]
    if args.shard:
        index, count = (int(x) for x in args.shard.split("/"))
        found = [one for at, one in enumerate(found) if at % count == index - 1]
    ran = {} if args.no_premise else exercised(args.bin, [(t, p, "-O2", m) for t, p in biggest for m in PROBE_MACHINES])
    unexercised = []
    for name, counts in ran.items():
        if counts is None:
            print(f"oracle {name}: not measurable (no perf, or a probe did not compile)")
        else:
            gain = counts[1] - counts[0]
            print(f"oracle {name}: {counts[0]} -> {counts[1]} instructions ({gain / counts[0]:+.4%}), {counts[2]} hook sites reached, on the {len(biggest)} largest programs at {' and '.join(PROBE_MACHINES)}")
            if 0 < gain < counts[0] * NOTABLE:
                print(f"  (under the old {NOTABLE:.2%} bar; a cheap oracle that ran is exercised)")
        if (counts is None or counts[2] < 1 or counts[1] <= counts[0]) and name not in args.allow_unexercised:
            unexercised.append(name)
    if unexercised:
        print("never exercised: " + ", ".join(unexercised) + " (a sweep where an oracle did not run proves nothing about it)")
        return 2
    if args.premise_only:
        print(f"every oracle ran\nwall {time.perf_counter() - started:.0f} s (premise)")
        return 0
    jobs = [(t, p, l, m) for t, p in found for l in args.levels.split(",") for m in args.machines.split(",")]
    with ThreadPoolExecutor(args.jobs) as pool:
        results = list(pool.map(lambda job: compile_one(job, args.bin), jobs))
    findings: dict[str, set[str]] = collections.defaultdict(set)
    silent = [] if args.no_reach else unreached(results, args.allow_unexercised)
    for (tool, source, level, machine), why, _ in results:
        if why:
            findings[why].add(f"{source.name} {level} {machine}")
    new, seen, unseen = judge(findings, expected)
    if args.shard:
        unseen = []  # a slice cannot see every kind the whole sweep sees
    # The line a reader needs first: did every compile run, and what failed that nothing expected.
    print(f"{len(results)} compiles / {len(new)} new finding kinds / {len(seen)} expected / {len(unseen)} expected entries paid (delete them)")
    for why, where in sorted(findings.items(), key=lambda item: -len(item[1])):
        print(f"{len(where):4}  {why}\n      e.g. {sorted(where)[:3]}")
    for one in unseen:
        print(f"paid: {one}: no longer happens; delete the entry from {args.expected}")
    if silent:
        print("never reached: " + ", ".join(silent) + " (no call site of the oracle was asked while it was on: a sweep that finds nothing proves nothing about it)")
        return 2
    print(f"wall {time.perf_counter() - started:.0f} s" + (f" (slice {args.shard}: {len(found)} programs)" if args.shard else ""))
    return 1 if new or unseen else 0


if __name__ == "__main__":
    sys.exit(main())
