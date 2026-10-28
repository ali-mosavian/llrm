"""`uv run --project tools python -m pytest tools/test_budgets.py`

Counted budgets, as ratios of two sizes of one generated program (scaling.py's axes at the gate's N and 2N, -O2): never an absolute
floor. A budget is what an event count or a step's instructions may grow by when the program doubles, and a regression that makes a
pass run once for each loop instead of once for the function reads as 4 where it read as 2. Every entry in budgets.toml above the
default is a debt: it names its issue and its owner, and the file only shrinks (docs/architecture/compile-time-spec.md, section 6).

The budgets are recorded on CI's build (release, CARGO_INCREMENTAL=0; the gate sets it). An incremental or dist binary counts the same step 9% lower or
higher, enough to cross the share: run it through `gate.py run`, or build with CARGO_INCREMENTAL=0.

Two kinds:
- events, exact and the same on any host (`LLRM_DEBUG=counts`: analysis misses, copies of a function, allocations, builders of
  dominators, loops, shapes): every count at 2N is at most `linear` times its count at N, plus `slack`, and the counts of a program of
  one function that make one thing per function do not grow at all;
- steps, which need an instruction counter (`LLRM_DEBUG=time`'s `[instr]` rows): each step with at least `floor_minstr` Minstr of work at 2N (any share of the compile: a share is a cliff that build flags and neighbours move) is
  at most `steps` times its own work at N. Both less the empty file's.
"""
import functools
import os
import math
import re
import subprocess
import sys
import tempfile
import tomllib
from concurrent.futures import ThreadPoolExecutor
from functools import cache
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import llrmbin  # noqa: E402

VSGCC = next((HERE.parent / "crates/target").glob("*/vsgcc"))
sys.path.insert(0, str(VSGCC))
import scaling  # noqa: E402
import scaling_gate  # noqa: E402

BUDGETS = tomllib.loads((HERE / "budgets.toml").read_text())
DEFAULT = BUDGETS["default"]
# Steps accepted as switching inside an axis's window (a threshold flipping between N/2 and 2N), each with its reason.
WINDOWS = BUDGETS.get("window", [])
# Programs of one function, where a count of copies, selections or allocations is a count of work done per function.
# A debt is paid only when its step is clearly under the limit: instruction counts move between commits and between hosts (x2.37 on one, under
# x2.23 on another: regalloc base on live), and a step at x2.30 against 2.3 read paid on one build and over on the next (lir twoaddr, lir coalesce).
PAID_MARGIN = 0.75
ONE_FUNCTION = ("straight", "mulconst", "branches", "live", "nest", "cells", "cases", "joins", "fjoins", "unrolled", "fanout", "equal")
PER_FUNCTION = ("function_clones", "function_forks", "machinings", "selections", "allocations", "cold_scans")
# The gate's N per axis, but nest 32 and unrolled 16 (their steps change regime between N=16 and 32, N=8 and 16: the readings below read 8x jumps, not
# slopes, #2260), and `branches` larger: its steps are small at the gate's 32 (analysis bounded is 1 Minstr there), and #1771 made
# `analysis bounded` quadratic in it (108 -> 982 Minstr from N=256 to 512) unseen by a size where the step is under the floor.
SIZES = {**scaling_gate.SIZES, "branches": 256, "nest": 32, "unrolled": 16}
AXES = tuple(SIZES)


DIP = 0.3  # the exponent N -> 2N is this much under N/2 -> N: the step may have settled, or be about to grow again


def dips(work_half: float, work_small: float, work_big: float) -> bool:
    """A step whose growth slowed after N/2 -> N: from three readings a settled step (a switch behind it) and a hump (it grows again) look alike."""
    return work_half >= 0.3 and work_small > 0 and exponent(work_small, work_big) < exponent(work_half, work_small) - DIP


def judged_ratio(work_small: float, work_big: float, work_quad: float | None) -> float:
    """The growth per doubling a step is judged on: the larger of N -> 2N and, where the step dips and a 4N reading exists, 2N -> 4N."""
    ratio = work_big / max(work_small, 1e-9)
    return ratio if work_quad is None else max(ratio, work_quad / max(work_big, 1e-9))


def test_a_dip_is_judged_by_the_exponent_it_grows_again_at():
    """nest intervals walk: 3.2 (N/2), 13.9 (N), 24.1 (2N), 86.9 (4N): x1.7 at N -> 2N, x3.6 at 2N -> 4N. Judged by the larger; a step with no 4N reading
    (no dip) by N -> 2N."""
    assert dips(3.2, 13.9, 24.1) and dips(0.9, 8.2, 17.5)
    assert judged_ratio(13.9, 24.1, 86.9) > 3.5
    assert judged_ratio(13.9, 24.1, None) < 1.8
    assert not dips(1.0, 2.0, 4.1)


@cache
def quad_steps(axis: str) -> dict[str, float]:
    """The steps of the compile at 4N: a third doubling, compiled only for the axes that hold a step whose growth dipped."""
    return parsed_steps(run(source_of(axis, 4 * SIZES[axis]), axis, "time"))


def still_rising(work_small: float, work_big: float) -> bool:
    """Whether a debt's step still grows: not under the limit by the margin. x2.30 against 2.3 is still rising."""
    return work_big > PAID_MARGIN * DEFAULT["steps"] * max(work_small, 1e-9)


def corroborated(limit: float, work_half: float, work_small: float, work_big: float, work_quad) -> bool:
    """Whether a step over `limit` from N to 2N is growth, not a step in the count: the doubling before it (N/2 -> N) or after it (2N -> 4N, read only
    if needed) is over too. The assembler's relaxation pass count flips once on a 16-byte shift (`assemble` x3.75 on cases, x1.8 at the sizes around it)."""
    over = lambda small, big: big > limit * max(small, 1e-9)  # noqa: E731
    return over(work_half, work_small) or over(work_big, work_quad())


def onset(work_small: float) -> bool:
    """A step whose work at N is under `onset_minstr` begins only at 2N: lazily built data (`siblings interference`, 0.0 -> 17.3) or a branch first taken
    (`peep facts exits`, 0.2 -> 10.2). Its ratio to a base of nothing is a number of the base, not a slope."""
    return work_small < DEFAULT["onset_minstr"]


def test_a_step_that_begins_between_the_two_sizes_has_no_slope_to_judge():
    """#2260 listed 'siblings interference' (0.0 -> 17.3, x1.7e10) and 'peep facts exits' (0.2 -> 10.2, x63) as debts: both are built on first use and
    absent at the small size."""
    assert onset(0.0) and onset(0.2)
    assert not onset(1.0) and not onset(11.3)


def read(work_big: float, total: float) -> bool:
    """Whether the test reads a step: its own work at 2N reaches the floor, whatever share of the compile that is."""
    return work_big >= DEFAULT["floor_minstr"]


def near_read(work_big: float, total: float) -> bool:
    """Within 10% under the floor: a step that hovers there is read on one build and not on the next."""
    return work_big >= 0.9 * DEFAULT["floor_minstr"]


def test_a_step_is_judged_by_its_own_slope_whatever_share_of_the_compile_it_is():
    """lir pressuresink (x3.3) sat at 2.03% of the compile against a 2% share: it entered the test at one commit (blamed on #2195) and read paid on
    an incremental build (1.72%). The same step at a 1.9% and a 2.1% share, with the same work, gets the same verdict."""
    work_small, work_big = 11.3, 37.6
    for total in (work_big / 0.019, work_big / 0.021):
        assert read(work_big, total) and still_rising(work_small, work_big)
    assert not read(4.0, 100.0), "under the floor in Minstr: not read, whatever its share"


def test_a_step_at_the_limit_is_not_paid_and_one_well_under_it_is():
    """lir twoaddr read x2.30 (60.3 -> 138.8): paid on one build, over on the next, and its debt was deleted as paid (#2191) then needed again."""
    assert still_rising(60.3, 138.8) and still_rising(60.3, 138.0)
    assert not still_rising(60.3, 100.0)


def test_a_step_that_reads_under_the_limit_on_another_host_is_not_paid():
    """regalloc base on live read x2.37 on the runner's host and under x2.23 on another: a margin of 0.97 deleted its row as paid there, and the
    runner refused (13:28). The step reads 2.2 here: still rising."""
    assert still_rising(36.9, 36.9 * 2.2) and still_rising(36.9, 36.9 * 2.0)
    assert not still_rising(36.9, 36.9 * 1.6)


def test_a_step_at_x2_read_three_percent_lower_on_another_tree_is_not_paid():
    """cases `mir decide@675` read 2.58 -> 5.22 (x2.02) on one tree and under x1.955 on the runner's (16:56, refused as paid): instruction counts are exact per tree and
    differ between trees by a few percent, so no second reading of one tree settles it. The band is wide enough for a step near x2 either way."""
    assert still_rising(2.58, 5.22) and still_rising(2.58, 2.58 * 1.9)


CC_TARGETS = {"m32": ("x86_32-wat", ["-march=i486"]), "m16": ("x86_16-wat", ["-march=i386"])}
# llrm-cc reads these from the environment, the first two with the tree's usual defaults.
CC_ENV = ("LLRM_OW", "LLRM_PIN_CLANG", "LLRM_CLANG_M16")


def compile_command(axis: str, source_path: Path, object_path: str) -> tuple[list[str], dict[str, str]]:
    """llrm-cc's argv and environment for one axis: the clang front end into llrm-llc (llrm-c, the Watcom front end, is being deleted, and its IR is not
    the IR of the compiler that stays), the 32-bit target at -march=i486 and the 16-bit at i386 (llrm-c rejects i286 at m16 and llrm-cc has no such flag)."""
    target, flags = CC_TARGETS["m16" if axis.endswith("-m16") else "m32"]
    env = {**{k: v for k, v in os.environ.items() if not k.startswith("LLRM_") or k in CC_ENV}, "LLRM_CC_TARGET": target}
    env.setdefault("LLRM_OW", str(Path.home() / "scratch/ow-src"))
    return [str(llrmbin.bin_dir() / "llrm-cc"), *flags, "-O2", "-c", "-o", object_path, str(source_path)], env


def run(source: str, axis: str, stage: str) -> str:
    """The compiler's stderr with `stage` on (`counts`, `time` or both) for `source`, compiled as the axis is."""
    with tempfile.TemporaryDirectory() as work:
        path = Path(work) / "a.c"
        path.write_text(source)
        command, env = compile_command(axis, path, llrmbin.OBJECT)
        env = {**env, "LLRM_DEBUG": stage, "LLRM_TIME_TOP": "100000"}
        with llrmbin.with_object(command) as command:
            done = subprocess.run(command, capture_output=True, text=True, env=env)
    assert done.returncode == 0, done.stderr[-500:]
    return done.stderr


def test_the_axes_are_compiled_by_llrm_cc(monkeypatch):
    """run() invoked llrm-c, the Watcom front end being deleted: every row of budgets.toml was measured on the IR of a compiler that does not stay (branches'
    lir_rename_asks read x2.65 there and x2.0 through llrm-cc)."""
    seen, real = {}, subprocess.run
    llrmbin.bin_dir()  # its stamp check runs the binaries: before the patch

    def fake(command, **kwargs):
        if "env" not in kwargs:
            return real(command, **kwargs)
        seen["command"], seen["env"] = command, kwargs["env"]
        return subprocess.CompletedProcess(command, 0, "", "")

    monkeypatch.setattr(subprocess, "run", fake)
    run("int x;", "equal", "counts")
    assert Path(seen["command"][0]).name == "llrm-cc", seen["command"]
    assert "-c" in seen["command"] and seen["env"]["LLRM_CC_TARGET"] == "x86_32-wat"
    run("int x;", "chain-m16", "counts")
    assert seen["env"]["LLRM_CC_TARGET"] == "x86_16-wat" and "-march=i386" in seen["command"]


def source_of(axis: str, size: int) -> str:
    return "" if size == 0 else scaling.AXES[axis.removesuffix("-m16")](size)


def parsed_counts(text: str) -> dict[str, int]:
    """Each event, and each analysis's misses (`analysis X`), as `LLRM_DEBUG=counts` prints them."""
    found = {}
    for line in text.splitlines():
        if (one := re.fullmatch(r"\[counts\] (\w+) (\d+)", line)):
            found[one[1]] = int(one[2])
        elif (one := re.fullmatch(r"\[counts\] analysis (.+?) (\d+) hits (\d+) misses", line)):
            found[f"analysis {one[1]}"] = int(one[3])
    return found


def parsed_steps(text: str) -> dict[str, float]:
    rows = re.findall(r"^\[instr\]\s+([\d.]+) (Minstr|Mcpu-ns) own\s+[\d.]+ \S+ total\s+\d+x (.+)$", text, re.M)
    if not rows or any(unit != "Minstr" for _, unit, _ in rows):
        pytest.skip("no instruction counter on this host: the rows are CPU time")
    return {name: float(own) for own, _, name in rows}


REGIME_JUMP = 1.0  # the exponent N -> 2N may exceed the one N/2 -> N by this much


def exponent(small: float, big: float) -> float:
    return math.log(big / small) / math.log(2)


def regime_change(work_half: float, work_small: float, work_big: float) -> float:
    """How much faster a step grows from N to 2N than from N/2 to N. A large number means something size-dependent switches on inside the window the
    slope is read in (spilling starts, an analysis is rerun per loop): the ratio is then the switch, not a slope. A step that settles before N (the
    exponent falls) is fine: that is the window being clear of the switch."""
    return exponent(work_small, work_big) - exponent(work_half, work_small)


def test_a_size_placed_across_a_regime_change_moves_the_exponent_up():
    """nest at N=16 read analysis loop-products 0.36 (N=8), 0.97, 8.16 (N=32): k 1.4 then 3.1, the x8.4 of a switch (#2260). At N=32 it is 0.97, 8.16,
    36.2: k 3.1 then 2.2, the switch behind it. A linear step does not move."""
    assert regime_change(0.36, 0.97, 8.16) > REGIME_JUMP
    assert regime_change(0.97, 8.16, 36.2) < 0
    assert abs(regime_change(1.0, 2.0, 4.1)) < 0.2


@cache
def half_steps(axis: str) -> dict[str, float]:
    """The steps of the compile at N/2: the premise's third point."""
    return parsed_steps(run(source_of(axis, SIZES[axis] // 2), axis, "time"))


@pytest.mark.parametrize("axis", AXES)
def test_no_step_switches_on_between_the_two_sizes_of_an_axis(axis):
    """The premise of the slope test: a step grows no faster from N to 2N than from N/2 to N (by more than REGIME_JUMP in the exponent). A regime
    change inside the window fails here loudly, naming the step, instead of reading as a slope (a debt) or as paid; the fix is a larger N."""
    _, _, small_text, big_text = measured(axis)
    empty, half = empty_steps(axis), half_steps(axis)
    small, big = parsed_steps(small_text), parsed_steps(big_text)
    switched = {}
    for name in big:
        wb, ws, wh = (max(table.get(name, 0.0) - empty.get(name, 0.0), 0.0) for table in (big, small, half))
        if wb < DEFAULT["floor_minstr"] or ws < 1.0 or wh < 0.3:
            continue
        if (jump := regime_change(wh, ws, wb)) > REGIME_JUMP:
            switched[name] = round(jump, 2)
    accepted = {row["name"] for row in WINDOWS if row["axis"] == axis}
    new = {name: jump for name, jump in switched.items() if name not in accepted}
    assert not new, f"{axis} at N={SIZES[axis]}: exponent N -> 2N exceeds N/2 -> N by: {new}: a regime change in the window; use a larger N"


@cache
def measured(axis: str) -> tuple[dict, dict, dict, dict]:
    """(events at N, at 2N, steps at N, at 2N less the empty file's) for one axis: one compile of each size, counts and steps."""
    n = SIZES[axis]
    with ThreadPoolExecutor(2) as pool:
        texts = list(pool.map(lambda size: run(source_of(axis, size), axis, "counts,time"), (n, 2 * n)))
    return parsed_counts(texts[0]), parsed_counts(texts[1]), texts[0], texts[1]


@cache
def empty_steps(axis: str) -> dict[str, float]:
    return parsed_steps(run("", axis, "time"))


def paid_debts(rows: list[dict], rising: set[str]) -> list[str]:
    """The debt rows whose step no longer rises. A row measured through a 4N reading (`(4N)` in its why: the step dips, then grows again) is judged by that third
    compile only on a host where the dip shows; on another it reads x1.45 and `lir machinesched` on joins was refused as paid (15:43) while it passed here."""
    return [one["name"] for one in rows if one["name"] not in rising and "(4N)" not in one["why"]]


def test_a_debt_measured_through_a_4n_reading_is_not_paid_by_a_2n_reading():
    """joins `lir machinesched` read 4.9 -> 7.1 (x1.45) on the runner's host, where the dip is not seen and the 4N compile is not made: its row read paid and
    #5744 was refused three times. A row without a 4N reading is paid as before."""
    hump = {"name": "lir machinesched", "why": "measured 4.9 -> 7.1 -> 32.4 (4N) Minstr (x4.53)"}
    plain = {"name": "lir jumps", "why": "measured 17.8 -> 47.5 Minstr (x2.66)"}
    assert paid_debts([hump, plain], set()) == ["lir jumps"]
    assert paid_debts([hump, plain], {"lir jumps"}) == []


def debts(kind: str, axis: str) -> list[dict]:
    return [one for one in BUDGETS.get("debt", []) if one["kind"] == kind and one["axis"] == axis]


def test_every_debt_names_its_issue_and_its_owner():
    unnamed = [f"{one['kind']} {one['axis']} {one['name']}" for one in BUDGETS.get("debt", []) if "UNASSIGNED" in (one["issue"], one["owner"]) or not one["why"]]
    assert not unnamed, f"a budget above the default has an issue and an owner: {unnamed}"


def allowed(kind: str, axis: str, name: str, default: float) -> tuple[float, str]:
    """The ratio a row may grow by, and the debt behind it: the file's entry for the row, else the default."""
    for one in BUDGETS.get("debt", []):
        if one["kind"] == kind and one["axis"] == axis and one["name"] == name:
            return one["ratio"], f" (debt: {one['issue']}, {one['owner']})"
    return default, ""


@pytest.mark.parametrize("axis", AXES)
def test_no_event_count_grows_faster_than_the_program_does(axis):
    small, big, _, _ = measured(axis)
    grown = []
    for name, count in big.items():
        ratio, debt = allowed("events", axis, name, DEFAULT["linear"])
        if count > ratio * small.get(name, 0) + DEFAULT["slack"]:
            grown.append(f"{name} {small.get(name, 0)} -> {count} (limit {ratio} per doubling{debt})")
    paid = [one["name"] for one in debts("events", axis) if big.get(one["name"], 0) <= DEFAULT["linear"] * small.get(one["name"], 0) + DEFAULT["slack"]]
    assert not grown, f"{axis} at N={SIZES[axis]} -> 2N: " + "; ".join(grown)
    assert not paid, f"{axis}: paid, so out of budgets.toml: {paid}"


@pytest.mark.parametrize("axis", ONE_FUNCTION)
def test_a_program_of_one_function_makes_one_thing_per_function_whatever_its_size(axis):
    small, big, _, _ = measured(axis)
    grown = [f"{name} {small.get(name, 0)} -> {big.get(name, 0)}" for name in PER_FUNCTION if big.get(name, 0) > small.get(name, 0) + DEFAULT["constant_slack"]]
    assert not grown, f"{axis}: " + "; ".join(grown)


@pytest.mark.parametrize("axis", AXES)
def test_no_step_grows_faster_than_the_program_does(axis):
    _, _, small_text, big_text = measured(axis)
    empty = empty_steps(axis)
    small, big = parsed_steps(small_text), parsed_steps(big_text)
    total = sum(max(value - empty.get(name, 0.0), 0.0) for name, value in big.items())
    grown, rising = [], set()
    half = half_steps(axis)
    work = lambda table, name: max(table.get(name, 0.0) - empty.get(name, 0.0), 0.0)  # noqa: E731
    dipping = {name for name in big if work(big, name) >= DEFAULT["floor_minstr"] and dips(work(half, name), work(small, name), work(big, name))}
    quad_of = functools.cache(lambda: quad_steps(axis))  # read once, and only for a step over its limit
    quad = quad_of() if dipping else {}
    for name, value in big.items():
        work_small, work_big = max(small.get(name, 0.0) - empty.get(name, 0.0), 0.0), max(value - empty.get(name, 0.0), 0.0)
        work_quad = work(quad, name) if name in dipping else None
        if onset(work_small):
            continue  # the step starts between N and 2N: no slope to judge from a base of nothing
        if not read(work_big, total):
            if near_read(work_big, total) and work_big > DEFAULT["steps"] * max(work_small, 1e-9):
                rising.add(name)  # a debt's step that dips under the floor is not paid by dipping
            continue
        ratio, debt = allowed("steps", axis, name, DEFAULT["steps"])
        widest = judged_ratio(work_small, work_big, work_quad)
        if widest > PAID_MARGIN * DEFAULT["steps"]:
            rising.add(name)
        if widest > ratio and (work_quad is not None or corroborated(ratio, work(half, name), work_small, work_big, lambda: work(quad_of(), name))):
            readings = f"{work_small:.1f} -> {work_big:.1f}" + (f" -> {work_quad:.1f} (4N)" if work_quad is not None else "")
            grown.append(f"{name} {readings} Minstr (x{widest:.2f}, limit {ratio} per doubling{debt})")
    paid = paid_debts(debts("steps", axis), rising)
    assert not grown, f"{axis} at N={SIZES[axis]} -> 2N: " + "; ".join(grown)
    assert not paid, f"{axis}: paid, so out of budgets.toml: {paid}"


def test_the_dominators_of_branches_grow_with_the_program_at_2n_and_at_4n():
    """`rotate` asked for the dominator tree after each of its loops and each ask applied the edits at O(N): `analysis dominators` read
    25.5 -> 83.4 Minstr (x3.27) from N=256 to 512. The tree is brought up to date when a reader asks (a lazy `DomTreeUpdater`)."""
    n = SIZES["branches"]
    empty = empty_steps("branches").get("analysis dominators", 0.0)
    work = [parsed_steps(run(source_of("branches", size), "branches", "time")).get("analysis dominators", 0.0) - empty for size in (n, 2 * n, 4 * n)]
    assert work[0] > 0.5, f"premise: the step ran and was counted, {work}"
    assert work[1] <= DEFAULT["steps"] * work[0] and work[2] <= DEFAULT["steps"] * work[1], f"N, 2N, 4N: {work[0]:.1f} {work[1]:.1f} {work[2]:.1f} Minstr (limit {DEFAULT['steps']} per doubling)"


def test_a_step_over_its_limit_at_one_doubling_only_is_not_growth_and_at_two_is():
    """cases `assemble` read 7.8 -> 29.3 Minstr (x3.75, limit 2.3) and flagged parity ee5a419f8a: the assembler's relaxation pass count flips on one 16-byte
    shift, x1.8 on both sides of it. One doubling over the limit flags nothing; the same step over at the doubling before or after it is growth."""
    flip = corroborated(2.3, 7.0, 7.8, 29.3, lambda: 29.3 * 1.8)
    assert not flip
    assert corroborated(2.3, 3.0, 7.8, 29.3, lambda: 29.3 * 1.8), "over at N/2 -> N too"
    assert corroborated(2.3, 7.0, 7.8, 29.3, lambda: 29.3 * 2.6), "over at 2N -> 4N too"


def test_the_assembler_relaxation_step_on_cases_is_a_window_step_not_a_regime_change():
    """cases `assemble` read 6.25 / 9.99 / 31.49 Minstr at N=64/128/256 net of the empty program 2.15 (jump 0.96, 1.0 on the runner's tree) and refused #5923 (round 2):
    a 16-byte shift of the object makes the assembler's relaxation loop take another pass, a step that moves with any code-size change. It is a window row."""
    assert regime_change(6.25 - 2.15, 9.99 - 2.15, 31.49 - 2.15) > 0.9
    assert any(row["axis"] == "cases" and row["name"] == "assemble" for row in WINDOWS)
