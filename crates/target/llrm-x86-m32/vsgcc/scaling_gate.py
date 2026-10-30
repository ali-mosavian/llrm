#!/usr/bin/env python3
"""Growth of llrm-c's cost with size: the cost at N/2, N and 2N on generated programs, per axis and level and per step of the compile.

    python3 scaling_gate.py [--jobs N]        print the costs as JSON (tools/measure.py compares them with the merge-base's)

Cost is user-space instructions (`perf stat`), less what llrm-c spends on an empty file: work done, so the ratio is the same on a
loaded host. tools/measure.py gates D = c(2N) - 3c(N) + 2c(N/2) = 1.5kN^2 of c = a + bN + kN^2: nil for fixed and linear work, so a
saving of either does not move it and a pass that goes quadratic does. Per step, llrm-c's LLRM_DEBUG=time
[instr] rows give each step's own instructions; a step with LOW or more of the work is read. Exit 77 without a counter.
The generated programs are scaling.py's AXES; SIZES holds the N per axis.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import levels_time  # noqa: E402
import scaling  # noqa: E402

# A step is read when it has LOW of the compile's own work at 2N (net of the empty file): a smaller step's count moves with run order,
# not growth. Its share is kept with its ratio; tools/measure.py fails only a step of tiers.toml [measure] high or more, so a step on
# the edge of any one share does not flip.
LOW = 0.015
# A step below LOW is still recorded from this much: tools/measure.py fails one whose cost rose 1.5x whatever its share, and a step that
# was not recorded in the base cannot be seen to rise (its share at 2N was under LOW, and then it grew).
RECORDED_MINSTR = 5.0
LEVELS = ("O1", "O2", "Os")
# N per axis, by one rule: the largest N whose 2N compile costs at most 12 G instructions at -O2 (about 3 s). Measured 2026-10-08
# on main b0342cdb2, whole-compile 2N/N at N, 2N, 4N, ...: no axis settles to a constant (the exponent drifts up with size, e.g.
# branches 2.92 2.84 2.75 3.46 over 16..256), so no size is "past the overhead"; the rule keeps the gate's cost flat and each
# axis as far up as it allows. A step whose ratio is a small-N artifact (branches' recolor reads 2.75 at 32->64, 1.9-2.2 at 64->256:
# the pairwise scan fills up to its 48-holder limit) is recorded as it reads; its budget says so, not that it is superlinear.
SIZES = {"functions": 64, "straight": 512, "branches": 32, "live": 64, "callers": 32, "chain": 32, "mulconst": 512, "nest": 16, "cells": 112}
# The rest of the generators, by the same rule (measured 2026-10-15, -O2, 2N: cases 128 -> 2.3 G, 256 -> 5.4 G, 512 -> 13.8 G; joins 64 -> 3.6 G,
# 128 -> 8.4 G, 256 -> 21 G; fjoins 64 -> 2.9 G, 128 -> 7.7 G; unrolled 16 -> 4.3 G, 32 -> 19 G). `cases` was in no table, and a step of it
# (`hir to mir`, 62 -> 230 -> 884 -> 3,480 -> 13,841 Minstr at -O1 over N = 64..1024, x4 per doubling) went quadratic unseen: every
# generator is an axis here, and a test says so.
SIZES |= {"cases": 128, "joins": 64, "fjoins": 64, "unrolled": 8}
# One value with N uses, and N equal expressions (use-list walks and replacements; measured 2026-10-19, -O2, 2N = 2048: ~3 G).
SIZES |= {"fanout": 1024, "equal": 2048}
# The axes that cross calls, again at -m16: another register file and calling convention, where a hang once hid (chain at N=7 never
# finished) while every -m32 axis passed.
SIZES |= {"chain-m16": 32, "callers-m16": 32}


def generated(axis: str, n: int) -> str:
    return scaling.AXES[axis.removesuffix("-m16")](n)


def commanded(axis: str, command):
    """`command`, for `axis`: -m16's axes compile with -m16."""
    if axis.endswith("-m16") and command is levels_time.command:
        return lambda compiler, level, source: levels_time.command(compiler, level, source, 16)
    return command


class NoCounter(Exception):
    pass


def count(command: list[str]) -> int:
    """User-space instructions of one run. A counter that reads nothing is unavailable, not zero work."""
    try:
        got = scaling.sample(command)[0]
    except (ValueError, KeyError, RuntimeError) as why:
        raise NoCounter(f"{levels_time.backend()} gave no instruction count ({why!r})")
    if got <= 0:
        raise NoCounter(f"{levels_time.backend()} counted 0 instructions")
    return got


def costs(axis: str, level: str, work: Path, command=levels_time.command, compiler: str = "llrm", counter=count) -> tuple[int, int, int]:
    """(cost at N/2, at N, at 2N), each less the empty file's, for one axis and level."""
    n = SIZES[axis]
    cost = {}
    for label, text in (("empty", ""), (n // 2, generated(axis, n // 2)), (n, generated(axis, n)), (2 * n, generated(axis, 2 * n))):
        source = work / f"{axis}_{level}_{label}.c"
        source.write_text(text)
        cost[label] = counter(commanded(axis, command)(compiler, level, source))
    return cost[n // 2] - cost["empty"], cost[n] - cost["empty"], cost[2 * n] - cost["empty"]


def ratio(axis: str, level: str, work: Path, command=levels_time.command, compiler: str = "llrm") -> float:
    """cost at 2N over cost at N, for one axis and level."""
    _, small, big = costs(axis, level, work, command, compiler)
    return big / small


def measure(jobs: int, axes=tuple(SIZES), levels=LEVELS) -> dict[str, tuple[int, int, int]]:
    with tempfile.TemporaryDirectory(dir=scaling.os.environ.get("CARGO_TARGET_DIR")) as tmp:
        todo = [(a, l) for a in axes for l in levels]
        with ThreadPoolExecutor(jobs) as pool:
            return dict(zip((f"{a} {l}" for a, l in todo), pool.map(lambda t: costs(*t, Path(tmp)), todo)))


RUN_AT = re.compile(r"@\d+$")
INSTR = re.compile(r"^\[instr\]\s+([\d.]+) (Minstr|Mcpu-ns) own\s+[\d.]+ \S+ total\s+\d+x (.+)$", re.M)


def own_work(command: list[str]) -> dict[str, float]:
    """Each step's own user-space Minstr from llrm-llc's `LLRM_DEBUG=time` [instr] rows (llrm-cc runs it). CPU time instead of a count is no count."""
    text = scaling.sample(command, {"LLRM_DEBUG": "time", "LLRM_TIME_TOP": "100000"})[2]
    rows = INSTR.findall(text)
    if not rows or any(unit != "Minstr" for _, unit, _ in rows):
        raise NoCounter("llrm-llc printed no instruction counts per step")
    # A pass that runs more than once, or is named by the line that makes it (`mir gvn@787`), is one step.
    steps: dict[str, float] = {}
    for own, _, name in rows:
        steps[RUN_AT.sub("", name)] = steps.get(RUN_AT.sub("", name), 0.0) + float(own)
    return steps


def pass_costs(axis: str, level: str, work: Path, command=levels_time.command, compiler: str = "llrm") -> dict[str, tuple[float, float, float, float]]:
    """Per step with LOW or more of the work: (own Minstr at N/2, at N, at 2N, and all steps' at 2N), each less the empty file's."""
    n = SIZES[axis]
    own = {}
    for label, text in (("empty", ""), (n // 2, generated(axis, n // 2)), (n, generated(axis, n)), (2 * n, generated(axis, 2 * n))):
        source = work / f"{axis}_{level}_{label}_p.c"
        source.write_text(text)
        own[label] = own_work(commanded(axis, command)(compiler, level, source))
    net = {name: big - own["empty"].get(name, 0.0) for name, big in own[2 * n].items()}
    whole = sum(v for v in net.values() if v > 0)
    out = {}
    for name, big in net.items():
        small = own[n].get(name, 0.0) - own["empty"].get(name, 0.0)
        if (big >= LOW * whole or big >= RECORDED_MINSTR) and small > 0:
            out[f"{axis} {level} {name}"] = (own[n // 2].get(name, 0.0) - own["empty"].get(name, 0.0), small, big, whole)
    return out


def pass_ratios(axis: str, level: str, work: Path, command=levels_time.command, compiler: str = "llrm") -> dict[str, tuple[float, float]]:
    """Per step: (its cost at 2N over its cost at N, its share of the work at 2N)."""
    return {k: (big / small, big / whole) for k, (_, small, big, whole) in pass_costs(axis, level, work, command, compiler).items()}


def measure_passes(jobs: int, axes=tuple(SIZES), levels=LEVELS) -> dict[str, tuple[float, float, float, float]]:
    """Each step's cost at N/2, N and 2N and the whole compile's. The compiler counts a step itself with `perf_event_open` (CPU time where
    it cannot): under callgrind there is no such counter, so no steps are measured and none are judged (the totals still are)."""
    if levels_time.backend() != "perf":
        return {}
    with tempfile.TemporaryDirectory(dir=scaling.os.environ.get("CARGO_TARGET_DIR")) as tmp:
        todo = [(a, l) for a in axes for l in levels]
        with ThreadPoolExecutor(jobs) as pool:
            return {k: v for got in pool.map(lambda t: pass_costs(*t, Path(tmp)), todo) for k, v in got.items()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--jobs", type=int, default=4)
    args = parser.parse_args()
    try:
        axes = {k: list(v) for k, v in measure(args.jobs).items()}
        passes = {k: [round(v, 3) for v in got] for k, got in measure_passes(args.jobs).items()}
    except NoCounter as why:
        print(f"SKIPPED: instruction counter unavailable ({why})")
        return 77
    print(json.dumps({"axes": axes, "passes": passes}, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
