#!/usr/bin/env python3
"""The scaling gate: llrm-c's cost at size 2N over its cost at N, per axis and level, against a stored budget.

    python3 scaling_gate.py             compare with tools/gate/scaling-budget.json; exit 1 on a change past SLACK, up or down
    python3 scaling_gate.py --refresh   rewrite the budget from this measurement
    python3 scaling_gate.py --ratios    print the ratios only

Cost is user-space instructions (`perf stat`), less what llrm-c spends on an empty file: work done, so the ratio is the
same on a loaded host. Linear work reads 2.0, a pass that goes quadratic pulls an axis to 3 and over. The budget is
what main costs now, so a pass that starts to grow faster fails, and a fix that makes growth slower fails until the
budget is refreshed in the same commit (the gcc-like target for every axis is about 2.1). Exit 77 without a counter.
Per step: llrm-c's LLRM_DEBUG=time [instr] rows give each step's own instructions; a step with 2% or more of the work
may more than double (2N/N above LINEAR) only at the ratio tools/gate/pass-budget.json records for it.
The generated programs are scaling.py's AXES; SIZES holds the N pair per axis.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import levels_time  # noqa: E402
import scaling  # noqa: E402

BUDGET = HERE.parents[3] / "tools/gate/scaling-budget.json"
PASS_BUDGET = HERE.parents[3] / "tools/gate/pass-budget.json"
FLOOR = 0.02  # share of the compile's own work at 2N (net of the empty file): a smaller step's count moves with run order, not growth
LINEAR = 2.1  # a pass above this at 2N/N is superlinear: it needs an entry in the pass budget (gcc's passes read up to about 2.1)
LEVELS = ("O1", "O2", "Os")
SIZES = {"functions": 64, "straight": 512, "branches": 32, "live": 64, "callers": 32, "chain": 32, "mulconst": 128}  # N: 2N compiles in about 2 s at -O2 (scaling.py's timings)
PASS_SLACK = 1.05  # a step's own count moves 2.7% at worst between runs (242 steps, 3 runs)
SLACK = 1.01  # the same build twice differs by 0.0014 at worst (21 ratios)


class NoCounter(Exception):
    pass


def count(command: list[str]) -> int:
    """User-space instructions of one run. A counter that reads nothing is unavailable, not zero work."""
    try:
        got = scaling.sample(command)[0]
    except (ValueError, KeyError) as why:
        raise NoCounter(f"perf gave no instruction count ({why!r})")
    if got <= 0:
        raise NoCounter("perf counted 0 instructions")
    return got


def ratio(axis: str, level: str, work: Path, command=levels_time.command, compiler: str = "llrm") -> float:
    """(cost at 2N - empty) / (cost at N - empty) for one axis and level."""
    n = SIZES[axis]
    cost = {}
    for label, text in (("empty", ""), (n, scaling.AXES[axis](n)), (2 * n, scaling.AXES[axis](2 * n))):
        source = work / f"{axis}_{level}_{label}.c"
        source.write_text(text)
        cost[label] = count(command(compiler, level, source))
    return (cost[2 * n] - cost["empty"]) / (cost[n] - cost["empty"])


def measure(jobs: int, axes=tuple(SIZES), levels=LEVELS) -> dict[str, float]:
    with tempfile.TemporaryDirectory(dir=scaling.os.environ.get("CARGO_TARGET_DIR")) as tmp:
        todo = [(a, l) for a in axes for l in levels]
        with ThreadPoolExecutor(jobs) as pool:
            return dict(zip((f"{a} {l}" for a, l in todo), pool.map(lambda t: ratio(*t, Path(tmp)), todo)))


INSTR = re.compile(r"^\[instr\]\s+([\d.]+) (Minstr|Mcpu-ns) own\s+[\d.]+ \S+ total\s+\d+x (.+)$", re.M)


def own_work(command: list[str]) -> dict[str, float]:
    """Each step's own user-space Minstr from llrm-c's `LLRM_DEBUG=time` [instr] rows. CPU time instead of a count is no count."""
    text = scaling.sample(command, {"LLRM_DEBUG": "time", "LLRM_TIME_TOP": "100000"})[2]
    rows = INSTR.findall(text)
    if not rows or any(unit != "Minstr" for _, unit, _ in rows):
        raise NoCounter("llrm-c printed no instruction counts per step")
    return {name: float(own) for own, _, name in rows}


def pass_ratios(axis: str, level: str, work: Path, command=levels_time.command, compiler: str = "llrm") -> dict[str, float]:
    """Per step: (own Minstr at 2N - at the empty file) / (at N - at the empty file), for steps with FLOOR of the work or more at 2N."""
    n = SIZES[axis]
    own = {}
    for label, text in (("empty", ""), (n, scaling.AXES[axis](n)), (2 * n, scaling.AXES[axis](2 * n))):
        source = work / f"{axis}_{level}_{label}_p.c"
        source.write_text(text)
        own[label] = own_work(command(compiler, level, source))
    net = {name: big - own["empty"].get(name, 0.0) for name, big in own[2 * n].items()}
    whole = sum(v for v in net.values() if v > 0)
    out = {}
    for name, big in net.items():
        small = own[n].get(name, 0.0) - own["empty"].get(name, 0.0)
        if big >= FLOOR * whole and small > 0:
            out[f"{axis} {level} {name}"] = big / small
    return out


def measure_passes(jobs: int, axes=tuple(SIZES), levels=LEVELS) -> dict[str, float]:
    with tempfile.TemporaryDirectory(dir=scaling.os.environ.get("CARGO_TARGET_DIR")) as tmp:
        todo = [(a, l) for a in axes for l in levels]
        with ThreadPoolExecutor(jobs) as pool:
            return {k: v for got in pool.map(lambda t: pass_ratios(*t, Path(tmp)), todo) for k, v in got.items()}


def compare_passes(budget: dict[str, float], now: dict[str, float], slack: float = PASS_SLACK, linear: float = LINEAR) -> tuple[list[str], list[str]]:
    """A step may more than double at 2N only at the ratio its budget entry records. An entry whose step now reads lower
    (or fell under FLOOR, or is gone) is a fix: the budget is refreshed in the same commit."""
    lines, bad = [], []
    for key, got in sorted(now.items()):
        allowed = max(budget.get(key, 0.0), linear)
        if got > allowed * slack:
            lines.append(f"{key}: {got:.3f} (allowed {allowed:.3f})")
            bad.append(f"{key}: 2N/N {got:.3f} > {allowed:.3f}: a pass more than doubles" + ("" if key in budget else f" (linear is {linear}; an entry in {PASS_BUDGET.name} records a known one)"))
    for key, was in sorted(budget.items()):
        got = now.get(key)
        if got is None or got < was / slack:
            bad.append(f"{key}: now {'under the floor or gone' if got is None else f'{got:.3f}'}, budget {was:.3f}: refresh the budget in this PR (python3 {Path(__file__).name} --refresh)")
    return lines, bad


def pass_budget(now: dict[str, float]) -> dict[str, float]:
    return {k: round(v, 3) for k, v in sorted(now.items()) if v > LINEAR}


def compare(budget: dict[str, float], now: dict[str, float], slack: float = SLACK) -> tuple[list[str], list[str]]:
    lines, bad = [], []
    for key in sorted(budget.keys() ^ now.keys()):
        bad.append(f"{key}: in the {'budget' if key in budget else 'measurement'} only")
    for key in sorted(budget.keys() & now.keys()):
        lines.append(f"{key}: 2N/N {now[key]:.3f} (budget {budget[key]:.3f})")
        if now[key] > budget[key] * slack:
            bad.append(f"{key}: grows faster, {now[key]:.3f} > {budget[key]:.3f}: a pass is superlinear where it was not")
        if now[key] < budget[key] / slack:
            bad.append(f"{key}: grows slower, {now[key]:.3f} < {budget[key]:.3f}: refresh the budget in this PR (python3 {Path(__file__).name} --refresh)")
    return lines, bad


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--refresh", action="store_true")
    parser.add_argument("--ratios", action="store_true")
    parser.add_argument("--jobs", type=int, default=4)
    args = parser.parse_args()
    try:
        now = {k: round(v, 3) for k, v in measure(args.jobs).items()}
        passes = {k: round(v, 3) for k, v in measure_passes(args.jobs).items()}
    except NoCounter as why:
        print(f"SKIPPED: instruction counter unavailable ({why})")
        return 77
    if args.ratios:
        print(json.dumps({"axes": now, "passes": passes}, indent=1))
        return 0
    if args.refresh:
        BUDGET.write_text(json.dumps(now, indent=0) + "\n")
        PASS_BUDGET.write_text(json.dumps(pass_budget(passes), indent=0) + "\n")
        print(f"budget: {len(now)} axes -> {BUDGET.name}, {len(pass_budget(passes))} superlinear steps -> {PASS_BUDGET.name}")
        return 0
    lines, bad = compare(json.loads(BUDGET.read_text()), now)
    print("\n".join(lines))
    more, worse = compare_passes(json.loads(PASS_BUDGET.read_text()), passes)
    print("\n".join(more))
    if bad or worse:
        print("SCALING:", *bad, *worse, sep="\n  ")
        return 1
    print(f"scaling within {SLACK} of {BUDGET.name} and {PASS_SLACK} of {PASS_BUDGET.name} ({len(passes)} steps)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
