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
- steps, which need an instruction counter (`LLRM_DEBUG=time`'s `[instr]` rows): each step with `share` of the compile's work at 2N is
  at most `steps` times its own work at N. Both less the empty file's.
"""
import os
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
# Programs of one function, where a count of copies, selections or allocations is a count of work done per function.
# A debt is paid only when its step is clearly under the limit: instruction counts move a few thousandths between commits, and a step at x2.30
# against 2.3 read paid on one build and over on the next (lir twoaddr, lir coalesce).
PAID_MARGIN = 0.97
ONE_FUNCTION = ("straight", "mulconst", "branches", "live", "nest", "cells", "cases", "joins", "fjoins", "unrolled")
PER_FUNCTION = ("function_clones", "function_forks", "machinings", "selections", "allocations")
# The gate's N per axis, but `branches` larger: its steps are small at the gate's 32 (analysis bounded is 1 Minstr there), and #1771 made
# `analysis bounded` quadratic in it (108 -> 982 Minstr from N=256 to 512) unseen by a size where the step is under the floor.
SIZES = {**scaling_gate.SIZES, "branches": 256}
AXES = tuple(SIZES)


def still_rising(work_small: float, work_big: float) -> bool:
    """Whether a debt's step still grows: not under the limit by the margin. x2.30 against 2.3 is still rising."""
    return work_big > PAID_MARGIN * DEFAULT["steps"] * max(work_small, 1e-9)


def read(work_big: float, total: float) -> bool:
    """Whether the test reads a step: it has the share of the compile's work and the floor."""
    return work_big >= DEFAULT["share"] * total and work_big >= DEFAULT["floor_minstr"]


def near_read(work_big: float, total: float) -> bool:
    """Within 10% under the share or the floor: a step that hovers there is read on one build and not on the next."""
    return work_big >= 0.9 * DEFAULT["share"] * total and work_big >= 0.9 * DEFAULT["floor_minstr"]


def test_a_debt_step_hovering_at_the_share_threshold_does_not_flip_the_result():
    """lir pressuresink was 2.03% of the compile (limit 2%): it entered the test at one commit (blamed on #2195) and could leave it, 'paid' by dipping."""
    total = 1850.0
    for work_big in (0.0205 * total, 0.0199 * total, 0.0195 * total):
        assert read(work_big, total) == (work_big >= 0.02 * total)
        assert near_read(work_big, total), "within 10% under the share: still held to its debt"
    assert not near_read(0.015 * total, total)


def test_a_step_at_the_limit_is_not_paid_and_one_well_under_it_is():
    """lir twoaddr read x2.30 (60.3 -> 138.8): paid on one build, over on the next, and its debt was deleted as paid (#2191) then needed again."""
    assert still_rising(60.3, 138.8) and still_rising(60.3, 138.0)
    assert not still_rising(60.3, 120.0)


def run(source: str, axis: str, stage: str) -> str:
    """The compiler's stderr with `stage` on (`counts`, `time` or both) for `source`, compiled as the axis is."""
    compiler = llrmbin.bin_dir() / "llrm-c"
    flags = ["-m16"] if axis.endswith("-m16") else ["-m32", "-march=i486"]
    with tempfile.TemporaryDirectory() as work:
        path = Path(work) / "a.c"
        path.write_text(source)
        env = {**{k: v for k, v in os.environ.items() if not k.startswith("LLRM_")}, "LLRM_DEBUG": stage, "LLRM_TIME_TOP": "100000"}
        done = subprocess.run([str(compiler), *flags, "-O2", "-o", os.devnull, str(path)], capture_output=True, text=True, env=env)
    assert done.returncode == 0, done.stderr[-500:]
    return done.stderr


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
    for name, value in big.items():
        work_small, work_big = max(small.get(name, 0.0) - empty.get(name, 0.0), 0.0), max(value - empty.get(name, 0.0), 0.0)
        if not read(work_big, total):
            if near_read(work_big, total) and work_big > DEFAULT["steps"] * max(work_small, 1e-9):
                rising.add(name)  # a debt's step that dips under the share is not paid by dipping
            continue
        ratio, debt = allowed("steps", axis, name, DEFAULT["steps"])
        if still_rising(work_small, work_big):
            rising.add(name)
        if work_big > ratio * max(work_small, 1e-9):
            grown.append(f"{name} {work_small:.1f} -> {work_big:.1f} Minstr (limit {ratio} per doubling{debt})")
    paid = [one["name"] for one in debts("steps", axis) if one["name"] not in rising]
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
