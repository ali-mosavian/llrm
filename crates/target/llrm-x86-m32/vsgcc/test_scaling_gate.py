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


STEPS = (
    "import sys; n = len(open(sys.argv[1]).read().splitlines())\n"
    "rows = {{'linear step': {linear}, 'quadratic step': {quadratic}, 'tiny step': 0.001 * n}}\n"
    "for name, v in rows.items(): print(f'[instr] {{v:.3f}} {unit} own {{v:.3f}} {unit} total 1x {{name}}', file=sys.stderr)\n"
)


def steps_compiler(linear="0.01 * n", quadratic="0.0002 * n * n", unit="Minstr"):
    """A 'compiler' printing the [instr] rows llrm-c does: one linear step, one quadratic, one too small to count."""
    return lambda compiler, level, source: [sys.executable, "-I", "-c", STEPS.format(linear=linear, quadratic=quadratic, unit=unit), str(source)]


def test_a_step_gone_quadratic_reads_four_and_is_over_the_linear_limit(tmp_path):
    got = gate.pass_ratios("straight", "O2", tmp_path, steps_compiler(), "s")
    assert got["straight O2 quadratic step"][0] == pytest.approx(4.0, abs=0.3)
    assert got["straight O2 linear step"][0] == pytest.approx(2.0, abs=0.1)
    assert "straight O2 tiny step" not in got  # under LOW of the work: its count moves with run order
    _, bad = gate.compare_passes({}, got)
    assert any("quadratic step" in line for line in bad) and not any("linear step" in line for line in bad), bad


def test_a_known_superlinear_step_passes_at_its_ratio_and_fails_above_and_below():
    budget = {"a O2 x": 3.0}
    assert gate.compare_passes(budget, {"a O2 x": (3.05, 0.1)})[1] == []
    assert any("more than doubles" in line for line in gate.compare_passes(budget, {"a O2 x": (3.5, 0.1)})[1])
    assert any("refresh the budget" in line for line in gate.compare_passes(budget, {"a O2 x": (2.5, 0.1)})[1])
    assert any("refresh the budget" in line for line in gate.compare_passes(budget, {})[1])  # fell under LOW or gone


def test_a_step_on_the_edge_of_the_share_floor_fails_neither_by_being_there_nor_by_being_gone():
    """callers -Os `analysis callee-effects` sat at 2.0% of the work: the same binary read fail, pass, pass, because a share of
    1.99% and 2.01% put it in and out of the measurement."""
    in_the_band = {"a O2 x": (3.4, (gate.LOW + gate.HIGH) / 2)}
    assert gate.compare_passes({}, in_the_band)[1] == []  # new, but not above HIGH
    assert gate.compare_passes({"a O2 x": 3.4}, in_the_band)[1] == []  # recorded, and still there
    assert gate.compare_passes({}, {"a O2 x": (3.4, gate.HIGH + 0.001)})[1]  # above HIGH and new: a superlinear step
    assert gate.pass_budget({"a O2 x": (3.4, gate.FLOOR - 0.001), "b O2 x": (3.4, gate.FLOOR)}) == {"b O2 x": 3.4}


def test_cpu_time_instead_of_instruction_counts_is_no_counter(tmp_path):
    """compile-time prints Mcpu-ns where the host has no counter: a time, not a count of work done."""
    source = tmp_path / "a.c"
    source.write_text("x\n")
    with pytest.raises(gate.NoCounter):
        gate.own_work(steps_compiler(unit="Mcpu-ns")("llrm", "O2", source))
    assert gate.own_work(steps_compiler()("llrm", "O2", source))["linear step"] == pytest.approx(0.01)


def test_the_pass_budget_holds_only_steps_above_linear():
    import json

    budget = json.loads(gate.PASS_BUDGET.read_text())
    assert budget and all(v > gate.LINEAR for v in budget.values())
