"""Compare an integer Newton fixed-point square root with x87 FSQRT.

This is an independent value experiment.  It is not connected to qbopt's
compiler, optimizer, runtime, or scoreboards.

For an unsigned Q(I.F) input whose stored integer is ``raw``, the stored
square-root result is the nearest integer to::

    sqrt(raw / 2**F) * 2**F == sqrt(raw * 2**F)

The Newton route therefore works entirely on the integer radicand
``raw << F``.  It uses a small normalized-mantissa seed table, performs
exactly two Newton divisions, corrects to the floor square root, and applies
the exact nearest-integer test.  The x87 route executes FILD/FSQRT/FISTP in a
small helper and returns the resulting stored integer.

Run the default 16.16 experiment with::

    uv run python -m tools.references.fixed_sqrt
"""

from __future__ import annotations

import argparse
import html
import math
import os
import platform
import random
import shutil
import struct
import subprocess
import tempfile
from collections.abc import Iterable, Sequence
from collections import Counter
from dataclasses import dataclass
from pathlib import Path


HERE = Path(__file__).resolve().parent
FSQRT_SOURCE = HERE / "fixed_sqrt_fsqrt.c"


def _ceil_sqrt(value: int) -> int:
    root = math.isqrt(value)
    return root if root * root == value else root + 1


# Index i represents a normalized radicand in [i/64, (i+1)/64).  The table
# stores an upper-bound square-root coefficient in Q8.  Only entries 64..255
# are used; retaining all 256 makes the runtime lookup direct.
SEED_Q8 = tuple(_ceil_sqrt((index + 1) * 1024) for index in range(256))


@dataclass(frozen=True)
class NewtonResult:
    raw: int
    downward_corrections: int
    upward_corrections: int

    @property
    def corrections(self) -> int:
        return self.downward_corrections + self.upward_corrections


@dataclass(frozen=True)
class RouteStats:
    mismatches: int
    max_raw_delta: int
    max_error_lsb: float
    mean_error_lsb: float
    max_relative_error_percent: float
    mean_relative_error_percent: float
    first_mismatches: tuple[tuple[int, int, int], ...]


@dataclass(frozen=True)
class ConvergenceStats:
    inputs: int
    exact_floor: int
    max_floor_delta: int
    mean_floor_delta: float
    p95_floor_delta: int
    p99_floor_delta: int
    max_percentage_error: float
    mean_percentage_error: float


def newton_seed(radicand: int) -> int:
    """Return an integer upper bound with about seven useful leading bits."""

    if radicand < 2:
        return radicand

    root_exponent = (radicand.bit_length() - 1) // 2
    if root_exponent < 3:
        return 1 << (root_exponent + 1)

    index = radicand >> (2 * root_exponent - 6)
    coefficient = SEED_Q8[index]
    if root_exponent >= 8:
        return coefficient << (root_exponent - 8)
    divisor = 1 << (8 - root_exponent)
    return (coefficient + divisor - 1) // divisor


def newton_estimate(radicand: int, rounds: int) -> int:
    """Return the upper square-root estimate after ``rounds`` Newton steps."""

    if radicand < 0:
        raise ValueError("square root radicand must be nonnegative")
    if rounds < 0:
        raise ValueError("Newton round count must be nonnegative")
    if radicand < 2:
        return radicand

    estimate = newton_seed(radicand)
    for _ in range(rounds):
        estimate = (estimate + radicand // estimate) // 2
    return estimate


def newton_nearest_sqrt(radicand: int, rounds: int = 2) -> NewtonResult:
    """Round sqrt(radicand) to nearest after the requested Newton steps.

    The final loops are not optional: Newton division gives an approximation,
    while the language operation needs a fully specified integer result.
    Their counts are returned so the experiment exposes the correction cost.
    """

    if radicand < 2:
        if rounds < 0:
            raise ValueError("Newton round count must be nonnegative")
        return NewtonResult(radicand, 0, 0)

    estimate = newton_estimate(radicand, rounds)

    down = 0
    while estimate * estimate > radicand:
        estimate -= 1
        down += 1

    up = 0
    while (estimate + 1) * (estimate + 1) <= radicand:
        estimate += 1
        up += 1

    remainder = radicand - estimate * estimate
    rounded = estimate + (remainder > estimate)
    return NewtonResult(rounded, down, up)


def exact_nearest_sqrt(radicand: int) -> int:
    """Exact oracle for nearest sqrt; a half-way case is impossible here."""

    if radicand < 0:
        raise ValueError("square root radicand must be nonnegative")
    root = math.isqrt(radicand)
    return root + (radicand - root * root > root)


def fixed_newton(raw: int, fraction_bits: int, rounds: int = 2) -> NewtonResult:
    if raw < 0:
        raise ValueError("fixed-point raw value must be nonnegative")
    if fraction_bits < 0:
        raise ValueError("fraction_bits must be nonnegative")
    return newton_nearest_sqrt(raw << fraction_bits, rounds)


def fixed_exact(raw: int, fraction_bits: int) -> int:
    if raw < 0:
        raise ValueError("fixed-point raw value must be nonnegative")
    if fraction_bits < 0:
        raise ValueError("fraction_bits must be nonnegative")
    return exact_nearest_sqrt(raw << fraction_bits)


def build_corpus(*, bits: int, fraction_bits: int, samples: int, exhaustive: int, seed: int) -> list[int]:
    """Build deterministic ordinary and rounding-boundary inputs."""

    if not 1 <= bits <= 32:
        raise ValueError("bits must be in 1..32")
    if not 0 <= fraction_bits < bits:
        raise ValueError("fraction_bits must be smaller than bits")
    if samples < 0 or exhaustive < 0:
        raise ValueError("sample counts must be nonnegative")

    maximum = (1 << bits) - 1
    values = set(range(min(maximum + 1, exhaustive)))
    values.update((0, 1, maximum))

    for exponent in range(bits):
        power = 1 << exponent
        for raw in (power - 1, power, power + 1):
            if 0 <= raw <= maximum:
                values.add(raw)

    generator = random.Random(seed)
    for _ in range(samples):
        values.add(generator.randrange(maximum + 1))

    # The nearest result changes where N crosses r^2+r+1.  Map randomly
    # selected output thresholds back into the fixed input domain and probe
    # both sides.  This is much more sensitive than random raw inputs alone.
    maximum_root = math.isqrt(maximum << fraction_bits) + 1
    scale = 1 << fraction_bits
    for _ in range(samples):
        root = generator.randrange(maximum_root + 1)
        threshold = root * root + root + 1
        raw_threshold = (threshold + scale - 1) // scale
        for raw in range(raw_threshold - 2, raw_threshold + 3):
            if 0 <= raw <= maximum:
                values.add(raw)

    return sorted(values)


def _x87_build_and_command(cc: str, output: Path) -> tuple[list[str], str]:
    machine = platform.machine().lower()
    system = platform.system()
    command = [cc, "-O2", "-Wall", "-Wextra", "-Werror"]

    if system == "Darwin" and machine in {"arm64", "aarch64"}:
        command.extend(("-arch", "x86_64"))
        runner = ["arch", "-x86_64", os.fspath(output)]
        label = "x87 FSQRT (x86_64 under Rosetta)"
    elif machine in {"x86_64", "amd64", "i386", "i686"}:
        runner = [os.fspath(output)]
        label = f"x87 FSQRT ({machine})"
    else:
        raise RuntimeError(f"cannot build the x87 helper on {system} {machine}")

    command.extend((os.fspath(FSQRT_SOURCE), "-o", os.fspath(output)))
    subprocess.run(command, check=True)
    return runner, label


def x87_fsqrt_values(radicands: Sequence[int], *, cc: str = "clang") -> tuple[list[int], str]:
    """Return actual FILD/FSQRT/FISTP results from the x87 helper."""

    compiler = shutil.which(cc)
    if compiler is None:
        raise RuntimeError(f"C compiler not found: {cc}")
    if any(value < 0 or value >= 1 << 63 for value in radicands):
        raise ValueError("x87 helper accepts radicands in 0..INT64_MAX")

    payload = struct.pack(f"={len(radicands)}Q", *radicands)
    with tempfile.TemporaryDirectory(prefix="fixed-sqrt-") as directory:
        runner, label = _x87_build_and_command(compiler, Path(directory) / "fsqrt")
        completed = subprocess.run(runner, input=payload, stdout=subprocess.PIPE, check=True)

    expected_size = len(radicands) * 8
    if len(completed.stdout) != expected_size:
        raise RuntimeError(f"x87 helper returned {len(completed.stdout)} bytes; expected {expected_size}")
    return list(struct.unpack(f"={len(radicands)}Q", completed.stdout)), label


def software_fsqrt_values(radicands: Iterable[int]) -> tuple[list[int], str]:
    """Clearly labeled fallback; this does not claim to execute FSQRT."""

    return [round(math.sqrt(value)) for value in radicands], "binary64 sqrt fallback"


def route_stats(
    raw_values: Sequence[int], radicands: Sequence[int], actual: Sequence[int], exact: Sequence[int]
) -> RouteStats:
    mismatches: list[tuple[int, int, int]] = []
    max_delta = 0
    errors = []
    relative_errors = []
    for raw, radicand, got, wanted in zip(raw_values, radicands, actual, exact, strict=True):
        delta = abs(got - wanted)
        max_delta = max(max_delta, delta)
        true_root = math.sqrt(radicand)
        error = abs(got - true_root)
        errors.append(error)
        if true_root:
            relative_errors.append(100 * error / true_root)
        if got != wanted and len(mismatches) < 8:
            mismatches.append((raw, got, wanted))
    mismatch_count = sum(got != wanted for got, wanted in zip(actual, exact, strict=True))
    return RouteStats(
        mismatch_count,
        max_delta,
        max(errors, default=0.0),
        sum(errors) / len(errors) if errors else 0.0,
        max(relative_errors, default=0.0),
        sum(relative_errors) / len(relative_errors) if relative_errors else 0.0,
        tuple(mismatches),
    )


def _nearest_rank(values: Sequence[int], percentile: int) -> int:
    if not values:
        return 0
    if not 0 <= percentile <= 100:
        raise ValueError("percentile must be in 0..100")
    ordered = sorted(values)
    index = (percentile * len(ordered) + 99) // 100 - 1
    return ordered[max(0, index)]


def convergence_stats(radicands: Sequence[int], rounds: int) -> ConvergenceStats:
    """Measure Newton convergence without hiding correction behind an oracle."""

    deltas = []
    percentage_errors = []
    for radicand in radicands:
        estimate = newton_estimate(radicand, rounds)
        floor = math.isqrt(radicand)
        if estimate < floor:
            raise AssertionError("upper-bound Newton estimate crossed below the floor root")
        delta = estimate - floor
        deltas.append(delta)
        true_root = math.sqrt(radicand)
        if true_root:
            percentage_errors.append(100 * abs(estimate - true_root) / true_root)
    return ConvergenceStats(
        inputs=len(deltas),
        exact_floor=sum(delta == 0 for delta in deltas),
        max_floor_delta=max(deltas, default=0),
        mean_floor_delta=sum(deltas) / len(deltas) if deltas else 0.0,
        p95_floor_delta=_nearest_rank(deltas, 95),
        p99_floor_delta=_nearest_rank(deltas, 99),
        max_percentage_error=max(percentage_errors, default=0.0),
        mean_percentage_error=(sum(percentage_errors) / len(percentage_errors) if percentage_errors else 0.0),
    )


def write_matrix_svg(
    path: Path,
    rows: Sequence[tuple[int, int, ConvergenceStats]],
    fsqrt_rows: Sequence[tuple[int, int, int, int]],
) -> None:
    """Write a dependency-free SVG of convergence versus fractional width."""

    fractions = sorted({fraction_bits for fraction_bits, _, _ in rows})
    if not fractions:
        raise ValueError("cannot plot an empty matrix")

    width = 960
    height = 700
    left = 88
    right = 32
    plot_width = width - left - right
    top_one = 112
    top_two = 414
    plot_height = 190
    colors = {1: "#d1495b", 2: "#00798c", 3: "#edae49"}
    strokes = {1: "", 2: ' stroke-dasharray="10 6"', 3: ' stroke-dasharray="2 5"'}
    radii = {1: 6, 2: 4.5, 3: 3}

    def x_position(fraction_bits: int) -> float:
        if len(fractions) == 1:
            return left + plot_width / 2
        low = fractions[0]
        high = fractions[-1]
        return left + (fraction_bits - low) * plot_width / (high - low)

    maximum_delta = max((stats.max_floor_delta for _, _, stats in rows), default=0)
    log_max = max(1.0, math.log10(maximum_delta + 1))

    def delta_y(value: int) -> float:
        return top_one + plot_height * (1 - math.log10(value + 1) / log_max)

    def percent_y(value: float) -> float:
        return top_two + plot_height * (1 - value / 100)

    by_round = {
        rounds: sorted(
            ((fraction_bits, stats) for fraction_bits, row_rounds, stats in rows if row_rounds == rounds),
            key=lambda item: item[0],
        )
        for rounds in (1, 2, 3)
    }
    fsqrt_mismatches = sum(mismatches for _, _, mismatches, _ in fsqrt_rows)
    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" role="img">',
        "<title>Fixed-point Newton square-root convergence</title>",
        "<desc>Maximum correction distance and percentage already at the floor root "
        "for one to three Newton rounds.</desc>",
        "<style>",
        "svg{background:#ffffff;color:#17212b;font-family:ui-sans-serif,system-ui,-apple-system,sans-serif}",
        ".grid{stroke:#d8dee5;stroke-width:1}.frame{fill:none;stroke:#7c8793;stroke-width:1}",
        ".axis{fill:currentColor;font-size:12px}.title{fill:currentColor;font-size:20px;font-weight:650}",
        ".subtitle{fill:currentColor;font-size:14px;font-weight:600}.note{fill:currentColor;font-size:12px}",
        ".series{fill:none;stroke-width:2.5;stroke-linecap:round;stroke-linejoin:round}"
        ".point{stroke:#fff;stroke-width:1.5}",
        "</style>",
        f'<rect width="{width}" height="{height}" fill="#ffffff"/>',
        '<text class="title" x="24" y="34">Fixed-point Newton convergence</text>',
        '<text class="note" x="24" y="57">Unsigned 32-bit input · normalized Q8 seed · exact final rounding</text>',
    ]

    legend_x = 590
    for offset, rounds in enumerate((1, 2, 3)):
        x = legend_x + offset * 112
        color = colors[rounds]
        svg.append(
            f'<line x1="{x}" y1="48" x2="{x + 24}" y2="48" stroke="{color}" '
            f'stroke-width="3" stroke-linecap="round"{strokes[rounds]}/>'
        )
        svg.append(f'<text class="axis" x="{x + 31}" y="52">{rounds} round</text>')

    panels = (
        (top_one, "Maximum decrement to floor root", "log₁₀(raw units + 1)"),
        (top_two, "Inputs already at floor root", "percent of corpus"),
    )
    for panel_top, title, y_label in panels:
        svg.append(f'<text class="subtitle" x="{left}" y="{panel_top - 27}">{title}</text>')
        svg.append(
            f'<text class="axis" text-anchor="middle" '
            f'transform="translate(22 {panel_top + plot_height / 2}) rotate(-90)">{y_label}</text>'
        )
        svg.append(f'<rect class="frame" x="{left}" y="{panel_top}" width="{plot_width}" height="{plot_height}"/>')
        for fraction_bits in fractions:
            x = x_position(fraction_bits)
            svg.append(
                f'<line class="grid" x1="{x:.2f}" y1="{panel_top}" x2="{x:.2f}" y2="{panel_top + plot_height}"/>'
            )
            svg.append(
                f'<text class="axis" text-anchor="middle" x="{x:.2f}" '
                f'y="{panel_top + plot_height + 20}">{fraction_bits}</text>'
            )
        svg.append(
            f'<text class="axis" text-anchor="middle" x="{left + plot_width / 2}" '
            f'y="{panel_top + plot_height + 42}">fraction bits</text>'
        )

    delta_ticks = [0]
    power = 0
    while 10**power <= maximum_delta:
        delta_ticks.append(10**power)
        power += 1
    for tick in dict.fromkeys(delta_ticks):
        y = delta_y(tick)
        label = f"{tick:,}"
        svg.append(f'<line class="grid" x1="{left}" y1="{y:.2f}" x2="{left + plot_width}" y2="{y:.2f}"/>')
        svg.append(f'<text class="axis" text-anchor="end" x="{left - 9}" y="{y + 4:.2f}">{label}</text>')

    for tick in (0, 25, 50, 75, 100):
        y = percent_y(tick)
        svg.append(f'<line class="grid" x1="{left}" y1="{y:.2f}" x2="{left + plot_width}" y2="{y:.2f}"/>')
        svg.append(f'<text class="axis" text-anchor="end" x="{left - 9}" y="{y + 4:.2f}">{tick}%</text>')

    for rounds, observations in by_round.items():
        color = colors[rounds]
        delta_points = " ".join(
            f"{x_position(fraction_bits):.2f},{delta_y(stats.max_floor_delta):.2f}"
            for fraction_bits, stats in observations
        )
        percent_points = " ".join(
            f"{x_position(fraction_bits):.2f},{percent_y(100 * stats.exact_floor / stats.inputs):.2f}"
            for fraction_bits, stats in observations
        )
        svg.append(f'<polyline class="series" stroke="{color}"{strokes[rounds]} points="{delta_points}"/>')
        svg.append(f'<polyline class="series" stroke="{color}"{strokes[rounds]} points="{percent_points}"/>')
        for fraction_bits, stats in observations:
            x = x_position(fraction_bits)
            exact_percent = 100 * stats.exact_floor / stats.inputs
            svg.append(
                f'<circle class="point" fill="{color}" cx="{x:.2f}" '
                f'cy="{delta_y(stats.max_floor_delta):.2f}" r="{radii[rounds]}">'
                f"<title>{rounds} round, F={fraction_bits}: max decrement {stats.max_floor_delta:,}</title></circle>"
            )
            svg.append(
                f'<circle class="point" fill="{color}" cx="{x:.2f}" '
                f'cy="{percent_y(exact_percent):.2f}" r="{radii[rounds]}">'
                f"<title>{rounds} round, F={fraction_bits}: {exact_percent:.3f}% already exact</title></circle>"
            )

    fsqrt_note = f"FSQRT mismatches against exact rounding: {fsqrt_mismatches:,}"
    svg.append(f'<text class="note" x="{left}" y="{height - 18}">{html.escape(fsqrt_note)}</text>')
    svg.append("</svg>")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(svg) + "\n", encoding="utf-8")


def write_error_svg(
    path: Path,
    rows: Sequence[tuple[int, int, ConvergenceStats]],
    final_rows: Sequence[tuple[int, RouteStats]],
) -> None:
    """Write percentage, percentile, and final quantization error plots."""

    fractions = sorted({fraction_bits for fraction_bits, _, _ in rows})
    if not fractions:
        raise ValueError("cannot plot an empty error matrix")

    width = 960
    height = 1080
    left = 104
    right = 32
    plot_width = width - left - right
    plot_height = 142
    panel_tops = (116, 354, 592, 830)
    colors = {1: "#d1495b", 2: "#00798c", 3: "#edae49"}
    strokes = {1: "", 2: ' stroke-dasharray="10 6"', 3: ' stroke-dasharray="2 5"'}
    radii = {1: 6, 2: 4.5, 3: 3}

    def x_position(fraction_bits: int) -> float:
        if len(fractions) == 1:
            return left + plot_width / 2
        return left + (fraction_bits - fractions[0]) * plot_width / (fractions[-1] - fractions[0])

    by_round = {
        rounds: sorted(
            ((fraction_bits, stats) for fraction_bits, row_rounds, stats in rows if row_rounds == rounds),
            key=lambda item: item[0],
        )
        for rounds in (1, 2, 3)
    }
    metric_values = (
        [stats.max_percentage_error for _, _, stats in rows],
        [stats.mean_percentage_error for _, _, stats in rows],
        [float(stats.p99_floor_delta) for _, _, stats in rows],
    )

    def log_scale(values: Sequence[float], top: int):
        positive = [value for value in values if value > 0]
        if not positive:
            return (lambda _: top + plot_height), [(0.0, "0")]
        low_power = math.floor(math.log10(min(positive)))
        high_power = math.ceil(math.log10(max(positive)))
        if low_power == high_power:
            low_power -= 1
        span = high_power - low_power

        def y_position(value: float) -> float:
            if value <= 0:
                return top + plot_height
            return top + plot_height * (high_power - math.log10(value)) / span

        step = max(1, math.ceil(span / 5))
        powers = list(range(low_power, high_power + 1, step))
        if powers[-1] != high_power:
            powers.append(high_power)
        ticks = [(0.0, "0")]
        ticks.extend((10.0**power, f"1e{power:+d}") for power in powers)
        return y_position, ticks

    scales = [log_scale(values, top) for values, top in zip(metric_values, panel_tops[:3], strict=True)]
    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" role="img">',
        "<title>Fixed-point Newton square-root error analysis</title>",
        "<desc>Relative convergence error, correction percentiles, and final stored-value error.</desc>",
        "<style>",
        "svg{background:#ffffff;color:#17212b;font-family:ui-sans-serif,system-ui,-apple-system,sans-serif}",
        ".grid{stroke:#d8dee5;stroke-width:1}.frame{fill:none;stroke:#7c8793;stroke-width:1}",
        ".axis{fill:currentColor;font-size:12px}.title{fill:currentColor;font-size:20px;font-weight:650}",
        ".subtitle{fill:currentColor;font-size:14px;font-weight:600}.note{fill:currentColor;font-size:12px}",
        ".series{fill:none;stroke-width:2.5;stroke-linecap:round;stroke-linejoin:round}"
        ".point{stroke:#fff;stroke-width:1.5}",
        "</style>",
        f'<rect width="{width}" height="{height}" fill="#ffffff"/>',
        '<text class="title" x="24" y="34">Fixed-point Newton error analysis</text>',
        '<text class="note" x="24" y="57">Pre-correction convergence and final nearest-value error</text>',
        '<text class="note" x="24" y="76">Percentage maxima include unavoidable integer quantization '
        "at small roots.</text>",
    ]
    legend_x = 590
    for offset, rounds in enumerate((1, 2, 3)):
        x = legend_x + offset * 112
        color = colors[rounds]
        svg.append(
            f'<line x1="{x}" y1="48" x2="{x + 24}" y2="48" stroke="{color}" '
            f'stroke-width="3" stroke-linecap="round"{strokes[rounds]}/>'
        )
        svg.append(f'<text class="axis" x="{x + 31}" y="52">{rounds} round</text>')

    panel_specs = (
        ("Maximum absolute percentage error vs sqrt(N)", "percent, log scale"),
        ("Mean absolute percentage error vs sqrt(N)", "percent, log scale"),
        ("99th-percentile decrement to floor root", "raw units, log scale"),
        ("Final correctly rounded value error", "raw output LSBs"),
    )
    for panel_top, (title, y_label) in zip(panel_tops, panel_specs, strict=True):
        svg.append(f'<text class="subtitle" x="{left}" y="{panel_top - 25}">{title}</text>')
        svg.append(
            f'<text class="axis" text-anchor="middle" '
            f'transform="translate(24 {panel_top + plot_height / 2}) rotate(-90)">{y_label}</text>'
        )
        svg.append(f'<rect class="frame" x="{left}" y="{panel_top}" width="{plot_width}" height="{plot_height}"/>')
        for fraction_bits in fractions:
            x = x_position(fraction_bits)
            svg.append(
                f'<line class="grid" x1="{x:.2f}" y1="{panel_top}" x2="{x:.2f}" y2="{panel_top + plot_height}"/>'
            )
            svg.append(
                f'<text class="axis" text-anchor="middle" x="{x:.2f}" '
                f'y="{panel_top + plot_height + 18}">{fraction_bits}</text>'
            )
        svg.append(
            f'<text class="axis" text-anchor="middle" x="{left + plot_width / 2}" '
            f'y="{panel_top + plot_height + 38}">fraction bits</text>'
        )

    convergence_attributes = (
        "max_percentage_error",
        "mean_percentage_error",
        "p99_floor_delta",
    )
    for attribute, (y_position, ticks) in zip(convergence_attributes, scales, strict=True):
        for value, label in ticks:
            y = y_position(value)
            svg.append(f'<line class="grid" x1="{left}" y1="{y:.2f}" x2="{left + plot_width}" y2="{y:.2f}"/>')
            svg.append(f'<text class="axis" text-anchor="end" x="{left - 9}" y="{y + 4:.2f}">{label}</text>')

        for rounds, observations in by_round.items():
            points = " ".join(
                f"{x_position(fraction_bits):.2f},{y_position(float(getattr(stats, attribute))):.2f}"
                for fraction_bits, stats in observations
            )
            color = colors[rounds]
            svg.append(f'<polyline class="series" stroke="{color}"{strokes[rounds]} points="{points}"/>')
            for fraction_bits, stats in observations:
                value = float(getattr(stats, attribute))
                svg.append(
                    f'<circle class="point" fill="{color}" cx="{x_position(fraction_bits):.2f}" '
                    f'cy="{y_position(value):.2f}" r="{radii[rounds]}"><title>'
                    f"{rounds} round, F={fraction_bits}: "
                    f"{value:.9g}</title></circle>"
                )

    final_top = panel_tops[-1]

    def final_y(value: float) -> float:
        return final_top + plot_height * (1 - value / 0.5)

    for tick in (0.0, 0.1, 0.2, 0.3, 0.4, 0.5):
        y = final_y(tick)
        svg.append(f'<line class="grid" x1="{left}" y1="{y:.2f}" x2="{left + plot_width}" y2="{y:.2f}"/>')
        svg.append(f'<text class="axis" text-anchor="end" x="{left - 9}" y="{y + 4:.2f}">{tick:.1f}</text>')
    for attribute, color, dash, label in (
        ("max_error_lsb", "#6f42c1", "", "max"),
        ("mean_error_lsb", "#2a9d8f", ' stroke-dasharray="7 5"', "mean"),
    ):
        points = " ".join(
            f"{x_position(fraction_bits):.2f},{final_y(getattr(stats, attribute)):.2f}"
            for fraction_bits, stats in final_rows
        )
        svg.append(f'<polyline class="series" stroke="{color}"{dash} points="{points}"/>')
        for fraction_bits, stats in final_rows:
            value = getattr(stats, attribute)
            svg.append(
                f'<circle class="point" fill="{color}" cx="{x_position(fraction_bits):.2f}" '
                f'cy="{final_y(value):.2f}" r="5"><title>F={fraction_bits}: {label} {value:.9f} LSB</title></circle>'
            )
    svg.append('<line x1="700" y1="805" x2="724" y2="805" stroke="#6f42c1" stroke-width="3"/>')
    svg.append('<text class="axis" x="731" y="809">maximum</text>')
    svg.append('<line x1="810" y1="805" x2="834" y2="805" stroke="#2a9d8f" stroke-width="3" stroke-dasharray="7 5"/>')
    svg.append('<text class="axis" x="841" y="809">mean</text>')
    svg.append(
        '<text class="note" x="104" y="1060">Corrected Newton and FSQRT produced identical stored values.</text>'
    )
    svg.append("</svg>")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(svg) + "\n", encoding="utf-8")


def run(args: argparse.Namespace) -> int:
    raw_values = build_corpus(
        bits=args.bits,
        fraction_bits=args.fraction_bits,
        samples=args.samples,
        exhaustive=args.exhaustive,
        seed=args.seed,
    )
    radicands = [raw << args.fraction_bits for raw in raw_values]
    exact = [exact_nearest_sqrt(value) for value in radicands]

    newton_results = [newton_nearest_sqrt(value, args.rounds) for value in radicands]
    newton_values = [result.raw for result in newton_results]
    correction_counts = Counter(result.corrections for result in newton_results)

    try:
        fsqrt_values, fsqrt_label = x87_fsqrt_values(radicands, cc=args.cc)
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        if args.require_x87:
            raise SystemExit(f"x87 FSQRT unavailable: {error}") from error
        fsqrt_values, fsqrt_label = software_fsqrt_values(radicands)
        fsqrt_label += f" ({error})"

    routes = (
        (
            f"{args.rounds}-round integer Newton",
            route_stats(raw_values, radicands, newton_values, exact),
        ),
        (fsqrt_label, route_stats(raw_values, radicands, fsqrt_values, exact)),
    )

    print(f"unsigned Q{args.bits - args.fraction_bits}.{args.fraction_bits}")
    print(f"inputs checked: {len(raw_values):,}")
    print("rounding: nearest stored fixed-point value")
    print()
    print("| route | mismatches vs exact | max raw delta | max error (LSB) |")
    print("|---|---:|---:|---:|")
    for name, stats in routes:
        print(f"| {name} | {stats.mismatches:,} | {stats.max_raw_delta} | {stats.max_error_lsb:.9f} |")

    histogram = ", ".join(
        f"{count} correction: {frequency:,}" for count, frequency in sorted(correction_counts.items())
    )
    print()
    print(f"Newton correction histogram: {histogram}")

    for name, stats in routes:
        if stats.first_mismatches:
            print()
            print(f"First {name} mismatches (input raw, route raw, exact raw):")
            for raw, got, wanted in stats.first_mismatches:
                print(f"  {raw:#x}  {got:#x}  {wanted:#x}")

    return int(any(stats.mismatches for _, stats in routes))


def parse_fraction_matrix(value: str) -> tuple[int, ...]:
    try:
        fractions = tuple(int(part) for part in value.split(","))
    except ValueError as error:
        raise argparse.ArgumentTypeError("fraction matrix must be comma-separated integers") from error
    if not fractions:
        raise argparse.ArgumentTypeError("fraction matrix must not be empty")
    return fractions


def run_matrix(args: argparse.Namespace) -> int:
    print(f"unsigned {args.bits}-bit fixed point; nearest stored result")
    print()
    print(
        "| fraction bits | Newton rounds | inputs | already floor | max decrement | "
        "p99 decrement | mean decrement | max percentage error |"
    )
    print("|---:|---:|---:|---:|---:|---:|---:|---:|")

    matrix_rows: list[tuple[int, int, ConvergenceStats]] = []
    fsqrt_rows: list[tuple[int, int, int, int]] = []
    final_rows: list[tuple[int, RouteStats]] = []
    for fraction_bits in args.fraction_matrix:
        raw_values = build_corpus(
            bits=args.bits,
            fraction_bits=fraction_bits,
            samples=args.samples,
            exhaustive=args.exhaustive,
            seed=args.seed,
        )
        radicands = [raw << fraction_bits for raw in raw_values]
        exact = [exact_nearest_sqrt(value) for value in radicands]

        for rounds in (1, 2, 3):
            stats = convergence_stats(radicands, rounds)
            matrix_rows.append((fraction_bits, rounds, stats))
            print(
                f"| {fraction_bits} | {rounds} | {stats.inputs:,} | {stats.exact_floor:,} "
                f"| {stats.max_floor_delta:,} | {stats.p99_floor_delta:,} | {stats.mean_floor_delta:.6f} "
                f"| {stats.max_percentage_error:.9g}% |"
            )

        try:
            fsqrt_values, _ = x87_fsqrt_values(radicands, cc=args.cc)
        except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
            if args.require_x87:
                raise SystemExit(f"x87 FSQRT unavailable: {error}") from error
            fsqrt_values, _ = software_fsqrt_values(radicands)
        stats = route_stats(raw_values, radicands, fsqrt_values, exact)
        fsqrt_rows.append((fraction_bits, len(raw_values), stats.mismatches, stats.max_raw_delta))
        final_rows.append((fraction_bits, stats))

    print()
    print("| fraction bits | FSQRT inputs | mismatches vs exact | max raw delta |")
    print("|---:|---:|---:|---:|")
    for fraction_bits, inputs, mismatches, max_delta in fsqrt_rows:
        print(f"| {fraction_bits} | {inputs:,} | {mismatches:,} | {max_delta} |")

    print()
    print("A decrement count is the distance from the Newton estimate to floor(sqrt(N)) before rounding.")
    print("The corrected Newton result is exact by construction; this table exposes the cost hidden by correction.")
    write_matrix_svg(args.plot, matrix_rows, fsqrt_rows)
    write_error_svg(args.error_plot, matrix_rows, final_rows)
    print(f"visualization: {args.plot.resolve()}")
    print(f"error visualization: {args.error_plot.resolve()}")
    return int(any(mismatches for _, _, mismatches, _ in fsqrt_rows))


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bits", type=int, default=32, help="unsigned storage width (default: 32)")
    parser.add_argument("--fraction-bits", type=int, default=16, help="fraction bits (default: 16)")
    parser.add_argument("--samples", type=int, default=100_000, help="random and boundary samples")
    parser.add_argument("--exhaustive", type=int, default=65_536, help="exhaustive low raw values")
    parser.add_argument("--seed", type=int, default=0x4D4F4445, help="PRNG seed")
    parser.add_argument("--rounds", type=int, default=2, choices=(1, 2, 3), help="Newton rounds (default: 2)")
    parser.add_argument("--matrix", action="store_true", help="compare 1-3 rounds across fractional widths")
    parser.add_argument(
        "--fraction-matrix",
        type=parse_fraction_matrix,
        default=(0, 8, 16, 24, 31),
        help="comma-separated fractional widths for --matrix",
    )
    parser.add_argument(
        "--plot",
        type=Path,
        default=HERE / "fixed-sqrt-matrix.svg",
        help="SVG output path for --matrix",
    )
    parser.add_argument(
        "--error-plot",
        type=Path,
        default=HERE / "fixed-sqrt-errors.svg",
        help="error-analysis SVG output path for --matrix",
    )
    parser.add_argument("--cc", default="clang", help="C compiler for the x87 helper")
    parser.add_argument("--require-x87", action="store_true", help="fail instead of using the binary64 fallback")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    return run_matrix(args) if args.matrix else run(args)


if __name__ == "__main__":
    raise SystemExit(main())
