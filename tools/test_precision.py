import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import precision  # noqa: E402


def test_a_float_row_against_clang_at_the_fast_default_is_refused():
    """A float row measured at x87 80-bit intermediates was set beside clang's SSE rows: a different answer, and a gap that was not the code's."""
    with pytest.raises(SystemExit, match="mandelbrot.*standard"):
        precision.require("mandelbrot", "double x = 1.0;", "fast", against_clang=True)
    precision.require("mandelbrot", "double x = 1.0;", "standard", against_clang=True)
    precision.require("mandelbrot", "double x = 1.0;", "fast", against_clang=True, allow_fast=True)


def test_an_integer_row_or_a_comparison_with_ourselves_needs_no_flag():
    precision.require("lru", "int x; /* a double in a comment */", "fast", against_clang=True)
    precision.require("mandelbrot", "float y;", "fast", against_clang=False)


def test_the_flag_and_label_say_which_precision_the_row_had(monkeypatch):
    monkeypatch.delenv(precision.ENV, raising=False)
    assert precision.flags("fast") == [] and precision.flags("standard") == ["-fexcess-precision=standard"]
    assert precision.label(precision.current()) == "excess-precision=fast"
    monkeypatch.setenv(precision.ENV, "standard")
    assert precision.current() == "standard"
    with pytest.raises(SystemExit):
        precision.current("double")


def test_the_bench_compiles_at_the_precision_it_is_told(monkeypatch):
    """A float row compared with clang at the fast default carried no flag and no label: the bench passes -fexcess-precision=standard when asked."""
    sys.path.insert(0, str(Path(__file__).parent / "bench"))
    import bench

    variant = type("V", (), {"language": "c"})()
    monkeypatch.delenv(precision.ENV, raising=False)
    assert "-fexcess-precision=standard" not in bench.compile_flags(variant, "O2")
    monkeypatch.setenv(precision.ENV, "standard")
    assert "-fexcess-precision=standard" in bench.compile_flags(variant, "O2")
