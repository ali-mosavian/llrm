"""scaling.py's instrument: a known-quadratic compiler must read as slope 2, and the generators must not collapse."""
import subprocess
import sys

import pytest

import levels_time
import scaling

STAND_IN = "import sys; n = len(open(sys.argv[1]).read().splitlines()); sum(range({work}))"


def stand_in(work):
    """A 'compiler' that spends `work` (a formula in n, the source's line count) in C-speed loop iterations."""
    return lambda compiler, level, source: [sys.executable, "-I", "-c", STAND_IN.format(work=work), str(source)]


def test_a_quadratic_stand_in_reads_as_slope_two_and_a_linear_one_as_one(tmp_path):
    """The slope came out of the raw instruction counts less the empty file's, not out of a fitted guess."""
    quadratic = scaling.run_axis("straight", [64, 128, 256, 512], 1, 1e12, tmp_path, stand_in("300 * n * n"), ("q",), ("O2",), profile=False)
    linear = scaling.run_axis("straight", [64, 128, 256, 512], 1, 1e12, tmp_path, stand_in("60000 * n"), ("l",), ("O2",), profile=False)
    assert scaling.exponents(quadratic["series"]["q O2"], quadratic["base"]["q O2"]["ins"])[0] == pytest.approx(2.0, abs=0.15)
    assert scaling.exponents(linear["series"]["l O2"], linear["base"]["l O2"]["ins"])[0] == pytest.approx(1.0, abs=0.15)


def test_a_series_stops_at_the_first_compile_over_the_limit(tmp_path):
    got = scaling.run_axis("straight", [16, 32, 64, 128], 1, 1.0, tmp_path, stand_in("1"), ("q",), ("O2",), profile=False)
    assert [r["n"] for r in got["series"]["q O2"]] == [16]


def test_slope_of_exact_powers():
    assert scaling.slope([1, 2, 4, 8], [3, 12, 48, 192]) == pytest.approx(2.0)
    assert scaling.slope([1, 2, 4], [5, 5, 5]) == pytest.approx(0.0)


@pytest.mark.parametrize("axis", scaling.AXES)
def test_the_generated_work_is_still_there_after_optimisation(axis, tmp_path):
    """A generator whose program folds away measures nothing: llrm -O2's MIR must grow with N (it was 0 growth for a constant chain)."""
    sizes = {}
    small, large = (8, 16) if axis == "nest" else (32, 64)  # a nest 64 deep is a minute's compile
    for n in (small, large):
        source = tmp_path / f"{axis}{n}.c"
        source.write_text(scaling.AXES[axis](n))
        sizes[n] = scaling.llrm_profile(source)
        assert sizes[n]["mir"], "llrm printed no [mir] line"
    assert sizes[large]["mir"] >= 1.6 * sizes[small]["mir"]
    assert sizes[large]["functions"] >= sizes[small]["functions"]


def test_parse_time_reads_own_ms_and_the_mir_line():
    text = "[mir] functions 3 instructions 180\n[time] by own time:\n[time]      1.500 ms own      2.000 ms total       2x lir peephole\n[time] nesting\n"
    assert scaling.parse_time(text) == {"steps": {"lir peephole": 1.5}, "functions": 3, "mir": 180}


def test_qcport_stubs_catch_the_asserts_with_padding_and_the_asm_blocks(tmp_path):
    """`typedef char rec_leaf_ok    [` (spaces before the bracket) slipped past the first pattern: 47 modules failed under gcc."""
    src, inc = tmp_path / "src", tmp_path / "inc"
    (src / "host").mkdir(parents=True)
    inc.mkdir()
    (inc / "DOS.H").write_bytes(b"int x;\x1a")
    (src / "host" / "a.c").write_text("typedef char rec_leaf_ok    [ sizeof(int) == 2 ? 1 : -1 ];\nvoid f(void) {\n    _asm { mov sp_now, sp }\n    __asm push ebx\n}\n", encoding="latin-1")
    sources, stubbed = scaling.qcport_tree(src, inc, tmp_path / "work")
    text = sources[0].read_text()
    assert "rec_leaf_ok" not in text and "asm" not in text
    assert (tmp_path / "work/inc/dos.h").read_bytes() == b"int x;"
    cc = subprocess.run(["gcc", "-m32", "-fsyntax-only", "-x", "c", str(sources[0])], capture_output=True, text=True)
    assert cc.returncode == 0, cc.stderr


def test_a_straight_statement_does_not_cost_millions_of_instructions(tmp_path):
    """16 statements multiplying by random 32-bit constants cost llrm -O0 749 M instructions (gcc 87 M): 8.6x gcc from isel's multiply search, not from size."""
    source = tmp_path / "s.c"
    source.write_text(scaling.AXES["straight"](16))
    assert scaling.measure(levels_time.command("llrm", "O0", source), 1)["ins"] < 150_000_000
