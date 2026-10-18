"""The per-merge tests of feat/parity: the selection fits 10 s of CPU however many crates a merge touches, an overrun or a failed step fails it, and the last line says which."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_smoke as ps  # noqa: E402


def clocked(costs):
    """A clock and steps on virtual time: each step advances the clock by its cost and answers `verdict`."""
    now = [0.0]
    steps = [(name, lambda _bins, cost=cost, verdict=verdict, name=name: (now.__setitem__(0, now[0] + cost), (verdict, name))[1], cost) for name, cost, verdict in costs]
    return (lambda: now[0]), steps


def test_the_declared_selection_fits_the_budget_whatever_the_merge_touches():
    """Every merge pays this: a step added without dropping another, or a crate's tests added on top, pushes the total past the 10 s the user set."""
    assert ps.BUDGET <= 10.0 and ps.BUDGET - ps.fixed_cost() >= ps.UNIT_FLOOR, (ps.fixed_cost(), "the fixed steps must leave the unit tests their floor")
    for crates in ([], ["a"], ["a", "b", "c"], [f"c{i}" for i in range(40)]):
        assert ps.declared(crates) <= ps.BUDGET + 1e-9, (len(crates), ps.declared(crates))


def test_a_set_within_the_budget_ends_in_pass():
    clock, steps = clocked([("a", 2.0, "ok"), ("b", 3.0, "ok")])
    ok, line = ps.run_steps(steps, Path("."), clock)
    assert ok and line.startswith("PARITY SMOKE PASS") and "5.0 s CPU of 10 s" in line


def test_a_set_over_the_budget_fails_whatever_its_steps_say():
    """A selection that grew to 14 s slows every merge of the branch: it must not read as a pass."""
    clock, steps = clocked([("a", 7.0, "ok"), ("b", 7.0, "ok")])
    ok, line = ps.run_steps(steps, Path("."), clock)
    assert not ok and line.startswith("PARITY SMOKE FAIL") and "over the budget: 14.0 s > 10 s" in line


def test_a_failed_step_fails_the_set_and_a_skipped_one_is_named_not_passed():
    clock, steps = clocked([("a", 1.0, "fail"), ("b", 1.0, "skipped")])
    ok, line = ps.run_steps(steps, Path("."), clock)
    assert not ok and "a: a" in line and "skipped b: b" in line
    clock, steps = clocked([("b", 1.0, "skipped")])
    ok, line = ps.run_steps(steps, Path("."), clock)
    assert ok and "skipped b: b" in line


def test_the_budget_is_cpu_time_not_the_wall_clock_of_a_loaded_host():
    """The same work took 4.2 s and 8.2 s of wall on this host at load 60: a wall budget fails merges for nothing."""
    import inspect

    assert inspect.signature(ps.run_steps).parameters["clock"].default is ps.cpu


def test_the_cheapest_ranked_tests_that_fit_a_crates_share_are_chosen():
    ranking = {"llrm-x": [("a", 0.1), ("b", 0.2), ("c", 0.5), ("d", 0.6)]}
    assert ps.unit_tests("llrm-x", 0.35, ranking) == ["a", "b"]
    assert ps.unit_tests("llrm-x", 0.01, ranking) == []
    assert ps.unit_tests("llrm-none", 5.0, ranking) == [], "no ranking: all its tests, cut at the share"


def test_the_ranking_is_read_from_the_report_time_listing_cheapest_first():
    out = "test a::slow ... ok <0.900s>\ntest a::fast ... ok <0.001s>\ntest a::bad ... FAILED\ntest a::mid ... ok <0.020s>\n"
    assert ps.ranked(out) == [("a::fast", 0.001), ("a::mid", 0.02), ("a::slow", 0.9)]


def test_a_changed_file_names_the_crate_it_belongs_to():
    assert ps.crate_of("crates/opt/llrm-transforms/src/gvn.rs") == ("llrm-transforms", "crates/opt/llrm-transforms")
    assert ps.crate_of("tools/measure.py") is None and ps.crate_of("crates/opt/nowhere/src/x.rs") is None
