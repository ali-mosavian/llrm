"""The per-merge smoke set of feat/parity: its steps' declared costs fit 10 s, an overrun or a failed step fails it, and the last line says which."""
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


def test_the_declared_cost_of_the_set_fits_the_budget():
    """Every merge pays this: a step added without dropping another pushes the total past the 10 s the user set."""
    assert sum(cost for _, _, cost in ps.STEPS) <= ps.BUDGET, [(name, cost) for name, _, cost in ps.STEPS]
    assert ps.BUDGET <= 10.0


def test_a_set_within_the_budget_ends_in_pass():
    clock, steps = clocked([("a", 2.0, "ok"), ("b", 3.0, "ok")])
    ok, line = ps.run_steps(steps, Path("."), clock)
    assert ok and line.startswith("PARITY SMOKE PASS") and "5.0 s of 10 s" in line


def test_a_set_over_the_budget_fails_whatever_its_steps_say():
    """A smoke set that grew to 14 s slows every merge of the branch: it must not read as a pass."""
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


def test_the_last_line_carries_pass_or_fail_for_the_merge_loop():
    clock, steps = clocked([("a", 1.0, "ok")])
    assert ps.run_steps(steps, Path("."), clock)[1].split()[:3] == ["PARITY", "SMOKE", "PASS"]
