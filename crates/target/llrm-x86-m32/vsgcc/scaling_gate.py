#!/usr/bin/env python3
"""The scaling gate: llrm-c's cost at size 2N over its cost at N, per axis and level, against a stored budget.

    python3 scaling_gate.py             compare with tools/gate/scaling-budget.json; exit 1 on a change past SLACK, up or down
    python3 scaling_gate.py --refresh   rewrite the entries of the budgets that moved past their tolerance (none if none did)
    python3 scaling_gate.py --resolve   after a merge conflict in the budgets: take origin/main's files, then --refresh
    python3 scaling_gate.py --ratios    print the ratios only

Cost is user-space instructions (`perf stat`), less what llrm-c spends on an empty file: work done, so the ratio is the
same on a loaded host. Linear work reads 2.0, a pass that goes quadratic pulls an axis to 3 and over. The budget is
what main costs now, so a pass that starts to grow faster fails, and a fix that makes growth slower fails until the
budget is refreshed in the same commit (the gcc-like target for every axis is about 2.1). Exit 77 without a counter.
Per step: llrm-c's LLRM_DEBUG=time [instr] rows give each step's own instructions; a step with 2.5% or more of the work (1.5-2.5% neither fails by being there nor by being gone)
may more than double (2N/N above LINEAR) only at the ratio tools/gate/pass-budget.json records for it.
The generated programs are scaling.py's AXES; SIZES holds the N pair per axis.
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

BUDGET = HERE.parents[3] / "tools/gate/scaling-budget.json"
PASS_BUDGET = HERE.parents[3] / "tools/gate/pass-budget.json"
# A step is gated by its share of the compile's own work at 2N (net of the empty file); a small step's count moves with run order,
# not growth, and a step sitting on the edge of any one share flips in and out between runs of the same binary. So there are three
# edges: a step under LOW is not looked at, one at FLOOR or more is recorded in the budget, and only one at HIGH or more can fail for
# being new. A step between LOW and HIGH fails neither by being there nor by being gone.
LOW = 0.015
FLOOR = 0.02
HIGH = 0.025
LINEAR = 2.1  # a pass above this at 2N/N is superlinear: it needs an entry in the pass budget (gcc's passes read up to about 2.1)
LEVELS = ("O1", "O2", "Os")
# N per axis, by one rule: the largest N whose 2N compile costs at most 12 G instructions at -O2 (about 3 s). Measured 2026-10-08
# on main b0342cdb2, whole-compile 2N/N at N, 2N, 4N, ...: no axis settles to a constant (the exponent drifts up with size, e.g.
# branches 2.92 2.84 2.75 3.46 over 16..256), so no size is "past the overhead"; the rule keeps the gate's cost flat and each
# axis as far up as it allows. A step whose ratio is a small-N artifact (branches' recolor reads 2.75 at 32->64, 1.9-2.2 at 64->256:
# the pairwise scan fills up to its 48-holder limit) is recorded as it reads; its budget says so, not that it is superlinear.
SIZES = {"functions": 64, "straight": 512, "branches": 32, "live": 64, "callers": 32, "chain": 32, "mulconst": 512}
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


def pass_ratios(axis: str, level: str, work: Path, command=levels_time.command, compiler: str = "llrm") -> dict[str, tuple[float, float]]:
    """Per step: ((own Minstr at 2N - at the empty file) / (at N - at the empty file), its share of the work at 2N), for steps with LOW or more."""
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
        if big >= LOW * whole and small > 0:
            out[f"{axis} {level} {name}"] = (big / small, big / whole)
    return out


def measure_passes(jobs: int, axes=tuple(SIZES), levels=LEVELS) -> dict[str, tuple[float, float]]:
    with tempfile.TemporaryDirectory(dir=scaling.os.environ.get("CARGO_TARGET_DIR")) as tmp:
        todo = [(a, l) for a in axes for l in levels]
        with ThreadPoolExecutor(jobs) as pool:
            return {k: v for got in pool.map(lambda t: pass_ratios(*t, Path(tmp)), todo) for k, v in got.items()}


def judged(was: float, read: tuple[float, float] | None, slack: float = PASS_SLACK) -> str:
    """What the gate and the refresh both say of a budget entry `was` for a step read as (ratio, share): "within" its tolerance,
    "up" past it, "down" past it, or "gone" (under LOW of the work). One decision, so a refresh always leaves the gate passing."""
    if read is None:
        return "gone"
    if read[0] > was * slack:
        return "up"
    return "down" if read[0] < was / slack else "within"


def compare_passes(budget: dict[str, float], now: dict[str, tuple[float, float]], slack: float = PASS_SLACK, linear: float = LINEAR) -> tuple[list[str], list[str]]:
    """A step of HIGH share or more may more than double at 2N only at the ratio its budget entry records. An entry whose step now reads
    lower is a fix: the budget is refreshed in the same commit. So is one whose step fell under LOW; one between LOW and HIGH is
    neither, wherever it was last time."""
    lines, bad = [], []
    for key, (got, share) in sorted(now.items()):
        if key not in budget and got > linear * slack and share >= HIGH:
            lines.append(f"{key}: {got:.3f} (allowed {linear:.3f})")
            bad.append(f"{key}: 2N/N {got:.3f} > {linear:.3f}: a pass more than doubles (an entry in {PASS_BUDGET.name} records a known one)")
    for key, was in sorted(budget.items()):
        state = judged(was, now.get(key), slack)
        if state == "up" and now[key][0] > max(was, linear) * slack:
            lines.append(f"{key}: {now[key][0]:.3f} (allowed {was:.3f})")
            bad.append(f"{key}: 2N/N {now[key][0]:.3f} > {was:.3f}: a pass more than doubles")
        elif state in ("down", "gone"):
            got = now.get(key)
            bad.append(f"{key}: now {'under the floor or gone' if got is None else f'{got[0]:.3f}'}, budget {was:.3f}: refresh the budget in this PR (python3 {Path(__file__).name} --refresh)")
    return lines, bad


def pass_budget(now: dict[str, tuple[float, float]]) -> dict[str, float]:
    return {k: round(v, 3) for k, (v, share) in sorted(now.items()) if v > LINEAR and share >= FLOOR}


def refreshed_axes(old: dict[str, float], now: dict[str, float], slack: float = SLACK) -> dict[str, float]:
    """The axis budget with the ratios that moved past `slack` rewritten and the rest as they were: a refresh that changes what
    the gate would not have noticed rewrites nothing, so two PRs' refreshes do not conflict on noise."""
    return {k: (old[k] if k in old and old[k] / slack <= v <= old[k] * slack else v) for k, v in sorted(now.items())}


def refreshed_passes(old: dict[str, float], now: dict[str, tuple[float, float]], slack: float = PASS_SLACK) -> dict[str, float]:
    """The pass budget `compare_passes` passes on this measurement, changing what it would not: an entry kept while `judged` says
    within, rewritten when the step moved and is still above LINEAR, dropped when it is gone or no longer above LINEAR; and a new
    superlinear step of FLOOR or more added."""
    new = pass_budget(now)
    out = {}
    for key in sorted(old.keys() | new.keys()):
        if key not in old:
            out[key] = new[key]
        elif (state := judged(old[key], now.get(key), slack)) == "within":
            out[key] = old[key]
        elif state != "gone" and now[key][0] > LINEAR:
            out[key] = round(now[key][0], 3)
    return out


def resolve(base: str = "origin/main") -> None:
    """Takes `base`'s budget files whole (a merge conflict in them is noise or somebody's refresh) and refreshes them again here."""
    files = [str(BUDGET.relative_to(BUDGET.parents[2])), str(PASS_BUDGET.relative_to(PASS_BUDGET.parents[2]))]
    subprocess.run(["git", "checkout", base, "--", *files], cwd=BUDGET.parents[2], check=True)
    subprocess.run(["git", "add", *files], cwd=BUDGET.parents[2], check=True)


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
    parser.add_argument("--resolve", nargs="?", const="origin/main", metavar="BASE", help="take BASE's budget files (a conflict in them), then --refresh")
    parser.add_argument("--ratios", action="store_true")
    parser.add_argument("--jobs", type=int, default=4)
    args = parser.parse_args()
    if args.resolve:
        resolve(args.resolve)
        args.refresh = True
    try:
        now = {k: round(v, 3) for k, v in measure(args.jobs).items()}
        passes = {k: (round(v, 3), round(share, 4)) for k, (v, share) in measure_passes(args.jobs).items()}
    except NoCounter as why:
        print(f"SKIPPED: instruction counter unavailable ({why})")
        return 77
    if args.ratios:
        print(json.dumps({"axes": now, "passes": passes}, indent=1))
        return 0
    if args.refresh:
        axes, steps = refreshed_axes(json.loads(BUDGET.read_text()), now), refreshed_passes(json.loads(PASS_BUDGET.read_text()), passes)
        moved = sum(axes.get(k) != v for k, v in json.loads(BUDGET.read_text()).items()), sum(steps.get(k) != v for k, v in json.loads(PASS_BUDGET.read_text()).items())
        BUDGET.write_text(json.dumps(axes, indent=0) + "\n")
        PASS_BUDGET.write_text(json.dumps(steps, indent=0) + "\n")
        print(f"budget: {moved[0]} of {len(axes)} axes moved -> {BUDGET.name}, {moved[1]} of {len(steps)} superlinear steps moved -> {PASS_BUDGET.name}")
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
