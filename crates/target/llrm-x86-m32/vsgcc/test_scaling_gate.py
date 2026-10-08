"""scaling_gate.py: a pass gone quadratic reads as 2N/N = 4, a linear one as 2, and neither direction of change passes unseen."""
import sys

import pytest

import scaling
import scaling_gate as gate

STAND_IN = "import sys; n = len(open(sys.argv[1]).read().splitlines()); sum(range({work}))"


def stand_in(work):
    """A 'compiler' spending `work` (a formula in n, the source's line count) in C-speed loop iterations."""
    return lambda compiler, level, source: [sys.executable, "-I", "-c", STAND_IN.format(work=work), str(source)]


def test_a_quadratic_compiler_reads_as_four_and_a_linear_one_as_two(tmp_path):
    """The ratio is of cost net of the empty file, so a fixed start-up cost does not pull a quadratic one towards 2."""
    quadratic = gate.ratio("straight", "O2", tmp_path, stand_in("300 * n * n + 5000000"), "q")
    linear = gate.ratio("straight", "O2", tmp_path, stand_in("60000 * n + 5000000"), "l")
    assert quadratic == pytest.approx(4.0, abs=0.25)
    assert linear == pytest.approx(2.0, abs=0.15)


def test_a_ratio_over_the_budget_fails_and_one_under_it_says_to_refresh():
    budget = {"a O2": 2.0, "b O2": 3.0}
    _, bad = gate.compare(budget, {"a O2": 2.6, "b O2": 3.0})
    assert any("a O2" in line and "superlinear" in line for line in bad), bad
    _, bad = gate.compare(budget, {"a O2": 2.0, "b O2": 2.2})
    assert any("b O2" in line and "refresh the budget" in line for line in bad), bad
    assert gate.compare(budget, {"a O2": 2.005, "b O2": 2.99})[1] == []


def test_an_axis_in_only_one_side_fails():
    assert gate.compare({"a O2": 2.0}, {"a O2": 2.0, "b O2": 2.0})[1]
    assert gate.compare({"a O2": 2.0, "b O2": 2.0}, {"a O2": 2.0})[1]


def test_a_counter_that_reads_zero_is_no_counter_not_free_work(monkeypatch):
    """perf prints `<not supported>` or 0 on a VM with no counters; a zero cost made every ratio 0/0."""
    monkeypatch.setattr(scaling, "sample", lambda *a, **k: (0, 0, ""))
    with pytest.raises(gate.NoCounter):
        gate.count(["true"])
    monkeypatch.setattr(scaling, "sample", lambda *a, **k: (_ for _ in ()).throw(ValueError("<not supported>")))
    with pytest.raises(gate.NoCounter):
        gate.count(["true"])


def test_every_axis_has_a_size_and_the_budget_names_each_axis_and_level():
    import json

    assert set(gate.SIZES) == set(scaling.AXES)
    assert set(json.loads(gate.BUDGET.read_text())) == {f"{a} {l}" for a in gate.SIZES for l in gate.LEVELS}
