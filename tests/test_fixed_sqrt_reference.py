"""Regression tests for the independent fixed-point square-root experiment."""

from __future__ import annotations

import math
import platform
import shutil
from pathlib import Path

import pytest

from tools.references import fixed_sqrt


MACHINE = platform.machine().lower()
X87_RUNNABLE = MACHINE in {"x86_64", "amd64", "i386", "i686"} or (
    platform.system() == "Darwin" and MACHINE in {"arm64", "aarch64"}
)


@pytest.mark.parametrize("fraction_bits", range(8))
def test_two_newton_steps_produce_exact_u8_fixed_results(fraction_bits: int) -> None:
    """The proposed two-step kernel must not differ by one raw output unit."""

    for raw in range(256):
        result = fixed_sqrt.fixed_newton(raw, fraction_bits)
        assert result.raw == fixed_sqrt.fixed_exact(raw, fraction_bits)


@pytest.mark.parametrize("rounds", (1, 2, 3))
def test_one_to_three_rounds_are_exact_after_correction(rounds: int) -> None:
    """Changing the convergence budget must not change the stored value."""

    for raw in range(256):
        result = fixed_sqrt.fixed_newton(raw, 7, rounds)
        assert result.raw == fixed_sqrt.fixed_exact(raw, 7)


def test_newton_matches_exact_at_u32_rounding_boundaries() -> None:
    """Random-only measurement can miss the points where the raw result changes."""

    raw_values = fixed_sqrt.build_corpus(bits=32, fraction_bits=16, samples=2_000, exhaustive=1_024, seed=7)
    for raw in raw_values:
        result = fixed_sqrt.fixed_newton(raw, 16)
        assert result.raw == fixed_sqrt.fixed_exact(raw, 16)


def test_nearest_integer_rule_on_either_side_of_threshold() -> None:
    """Correction to floor alone would bias every non-square result downward."""

    root = 12345
    threshold = root * root + root + 1
    assert fixed_sqrt.exact_nearest_sqrt(threshold - 1) == root
    assert fixed_sqrt.exact_nearest_sqrt(threshold) == root + 1
    assert fixed_sqrt.newton_nearest_sqrt(threshold - 1).raw == root
    assert fixed_sqrt.newton_nearest_sqrt(threshold).raw == root + 1


def test_seed_is_an_upper_bound_for_representative_u63_radicands() -> None:
    """Starting below the root invalidates the bounded downward correction."""

    values = [2, 3, 4, 7, 8, 15, 16, (1 << 63) - 1]
    values.extend((raw << 16) for raw in range(1, 100_000, 997))
    for value in values:
        seed = fixed_sqrt.newton_seed(value)
        assert seed >= math.isqrt(value)
        assert seed > 0


def test_each_newton_round_reduces_distance_to_floor() -> None:
    """The round comparison must expose convergence rather than final correction."""

    radicands = [raw << 31 for raw in range(1, 10_000, 97)]
    previous = None
    for rounds in (1, 2, 3):
        stats = fixed_sqrt.convergence_stats(radicands, rounds)
        if previous is not None:
            assert stats.max_floor_delta <= previous.max_floor_delta
            assert stats.mean_floor_delta <= previous.mean_floor_delta
        previous = stats


def test_fraction_matrix_parser() -> None:
    assert fixed_sqrt.parse_fraction_matrix("0,8,16,24,31") == (0, 8, 16, 24, 31)


def test_matrix_visualization_contains_each_round(tmp_path: Path) -> None:
    """The matrix command must leave a readable visualization, not only console rows."""

    rows = [
        (
            fraction_bits,
            rounds,
            fixed_sqrt.ConvergenceStats(
                inputs=100,
                exact_floor=100 - rounds,
                max_floor_delta=4 - rounds,
                mean_floor_delta=0.1,
                p95_floor_delta=1,
                p99_floor_delta=2,
                max_percentage_error=0.01,
                mean_percentage_error=0.001,
            ),
        )
        for fraction_bits in (0, 16, 31)
        for rounds in (1, 2, 3)
    ]
    output = tmp_path / "matrix.svg"
    fixed_sqrt.write_matrix_svg(output, rows, [(0, 100, 0, 0), (16, 100, 0, 0), (31, 100, 0, 0)])
    svg = output.read_text()
    assert "Fixed-point Newton convergence" in svg
    assert "1 round" in svg
    assert "2 round" in svg
    assert "3 round" in svg
    assert 'stroke-dasharray="10 6"' in svg
    assert 'stroke-dasharray="2 5"' in svg
    assert "FSQRT mismatches against exact rounding: 0" in svg


def test_error_visualization_contains_percentage_and_final_error(tmp_path: Path) -> None:
    """The additional plot must expose relative, percentile, and final error."""

    rows = [
        (
            fraction_bits,
            rounds,
            fixed_sqrt.ConvergenceStats(100, 90, 4, 0.1, 1, 2, 0.01 / rounds, 0.001 / rounds),
        )
        for fraction_bits in (0, 16, 31)
        for rounds in (1, 2, 3)
    ]
    route = fixed_sqrt.RouteStats(0, 0, 0.49, 0.24, 10.0, 0.01, ())
    output = tmp_path / "errors.svg"
    fixed_sqrt.write_error_svg(output, rows, [(0, route), (16, route), (31, route)])
    svg = output.read_text()
    assert "Maximum absolute percentage error" in svg
    assert "99th-percentile decrement" in svg
    assert "Final correctly rounded value error" in svg
    assert 'stroke-dasharray="10 6"' in svg
    assert 'stroke-dasharray="2 5"' in svg
    assert "Corrected Newton and FSQRT produced identical stored values" in svg


@pytest.mark.skipif(
    not X87_RUNNABLE or shutil.which("clang") is None,
    reason="the value reference requires Clang and either an x86 host or Rosetta",
)
def test_actual_x87_fsqrt_matches_exact_fixed_values() -> None:
    """A host sqrt substitute must not be reported as evidence about FSQRT."""

    raw_values = [0, 1, 2, 3, 0xFFFF, 0x10000, 0x10001, 0x7FFFFFFF, 0xFFFFFFFF]
    radicands = [raw << 16 for raw in raw_values]
    actual, label = fixed_sqrt.x87_fsqrt_values(radicands)
    assert label.startswith("x87 FSQRT")
    assert actual == [fixed_sqrt.exact_nearest_sqrt(value) for value in radicands]
