"""scaling_gate.py: a pass gone quadratic reads as 2N/N = 4, a linear one as 2, and neither direction of change passes unseen."""
import sys
from pathlib import Path

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


def test_a_counter_that_reads_zero_is_no_counter_not_free_work(monkeypatch):
    """perf prints `<not supported>` or 0 on a VM with no counters; a zero cost made every ratio 0/0."""
    monkeypatch.setattr(scaling, "sample", lambda *a, **k: (0, 0, ""))
    with pytest.raises(gate.NoCounter):
        gate.count(["true"])
    monkeypatch.setattr(scaling, "sample", lambda *a, **k: (_ for _ in ()).throw(ValueError("<not supported>")))
    with pytest.raises(gate.NoCounter):
        gate.count(["true"])


STEPS = (
    "import sys; n = len(open(sys.argv[1]).read().splitlines())\n"
    "rows = {{'linear step': {linear}, 'quadratic step': {quadratic}, 'tiny step': 0.001 * n}}\n"
    "for name, v in rows.items(): print(f'[instr] {{v:.3f}} {unit} own {{v:.3f}} {unit} total 1x {{name}}', file=sys.stderr)\n"
)


def steps_compiler(linear="0.01 * n", quadratic="0.0002 * n * n", unit="Minstr"):
    """A 'compiler' printing the [instr] rows llrm-c does: one linear step, one quadratic, one too small to count."""
    return lambda compiler, level, source: [sys.executable, "-I", "-c", STEPS.format(linear=linear, quadratic=quadratic, unit=unit), str(source)]


def test_a_step_gone_quadratic_reads_four_and_a_linear_one_two(tmp_path):
    got = gate.pass_ratios("straight", "O2", tmp_path, steps_compiler(), "s")
    assert got["straight O2 quadratic step"][0] == pytest.approx(4.0, abs=0.3)
    assert got["straight O2 linear step"][0] == pytest.approx(2.0, abs=0.1)
    assert "straight O2 tiny step" not in got  # under LOW of the work: its count moves with run order


def test_cpu_time_instead_of_instruction_counts_is_no_counter(tmp_path):
    """compile-time prints Mcpu-ns where the host has no counter: a time, not a count of work done."""
    source = tmp_path / "a.c"
    source.write_text("x\n")
    with pytest.raises(gate.NoCounter):
        gate.own_work(steps_compiler(unit="Mcpu-ns")("llrm", "O2", source))
    assert gate.own_work(steps_compiler()("llrm", "O2", source))["linear step"] == pytest.approx(0.01)


def test_the_costs_read_back_give_a_second_difference_of_fixed_and_linear_work_nil_and_of_quadratic_not(tmp_path):
    """The ratio 2N/N rose when linear work got cheaper, and c(2N) - 2c(N) when a fixed cost went: the gate compares
    c(2N) - 3c(N) + 2c(N/2), which cancels both."""
    second = lambda c: c[2] - 3 * c[1] + 2 * c[0]
    fixed_and_linear = gate.costs("straight", "O2", tmp_path, stand_in("60000 * n + 5000000"), "l")
    cheaper = gate.costs("straight", "O2", tmp_path, stand_in("30000 * n + 1000000"), "c")
    quadratic = gate.costs("straight", "O2", tmp_path, stand_in("60000 * n + 300 * n * n + 5000000"), "q")
    assert abs(second(fixed_and_linear)) < 0.03 * fixed_and_linear[2] and abs(second(cheaper)) < 0.03 * cheaper[2]
    assert second(quadratic) > 0.1 * quadratic[2]


def test_the_16_bit_axes_compile_with_m16_and_the_others_with_m32():
    """Every axis ran -m32 only, and `chain` at N=7 with -m16 never finished unseen. The interprocedural axes run at both."""
    import levels_time
    sixteen = gate.commanded("chain-m16", levels_time.command)("llrm", "O2", Path("x.c"))
    assert "-m16" in sixteen and "-m32" not in sixteen
    assert "-m32" in gate.commanded("chain", levels_time.command)("llrm", "O2", Path("x.c"))
    assert {"chain-m16", "callers-m16"} <= set(gate.SIZES)
    assert scaling.AXES["chain"](3) == gate.generated("chain-m16", 3)


def test_a_count_does_not_inherit_the_callers_llrm_variables(monkeypatch):
    """A caller's LLRM_CHECK_* or LLRM_VERIFY reached the compiler under measurement and added work to the step it checks: regparm16's
    branch read 4.5 Minstr over in 'lir peephole' at every size, which failed measure on a flat constant."""
    monkeypatch.setenv("LLRM_CHECK_FOO", "1")
    monkeypatch.setenv("LLRM_BIN", "/kept")
    seen = scaling.sample([sys.executable, "-I", "-c", "import os, sys; print(sorted(k for k in os.environ if k.startswith('LLRM_')), file=sys.stderr)"], {"LLRM_DEBUG": "time"})[2]
    assert "LLRM_CHECK_FOO" not in seen and "LLRM_BIN" in seen and "LLRM_DEBUG" in seen, seen


def test_the_nest_axis_is_a_loop_nest_as_deep_as_it_says_and_the_gate_sizes_it():
    """gap32's recursive inlining nested loops deeply and rectwo -O2 went 65 M -> 792 M: hoist, lsr, peephole, jumps and the allocator are
    superlinear in nesting depth, which no axis measured (2N/N of a nest 16 deep is 4.3, D/c(2N) 0.44)."""
    text = scaling.AXES["nest"](5)
    assert text.count("for (") == 5
    depths = [len(line) - len(line.lstrip()) for line in text.splitlines() if line.lstrip().startswith("for (")]
    assert depths == sorted(depths) and len(set(depths)) == 5, depths
    assert gate.SIZES["nest"] == 16
