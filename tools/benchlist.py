"""Which directories are benchmarks: those holding a bench.toml (bench/NAME and bench/parity/NAME).

The one place that says so. Tools that globbed `bench/*` took every directory there for a program: `bench/waivers` (#1820) was a benchmark
called `waivers` to identical.py, vsgcc's programs, test_scaling_gate, test_compile_cost and tests/run.rs.
"""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BENCH = ROOT / "bench"


def directories(bench: Path = BENCH, parity: bool = True) -> list[Path]:
    """Each benchmark's directory, sorted: bench/NAME, and with `parity` bench/parity/NAME."""
    found = sorted(bench.glob("*/bench.toml"))
    if parity:
        found += sorted(bench.glob("parity/*/bench.toml"))
    return [one.parent for one in found]


def sources(suffix: str, bench: Path = BENCH, parity: bool = True) -> list[Path]:
    """The `suffix` files (".c") in the benchmarks' directories."""
    return [path for directory in directories(bench, parity) for path in sorted(directory.glob(f"*{suffix}"))]
