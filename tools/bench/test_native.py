"""The x86_64 rows: which programs run natively, and that a row is a count of the program's own run."""

import shutil
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))
import native  # noqa: E402

PROGRAM = "extern void report(long value);\nlong bench_f(int n) { long s = 0; for (int i = 0; i < n; i++) s += i * i; return s; }\nint main(void) { report(bench_f(20)); return 0; }\n"


def program(tmp_path, name="f", source=PROGRAM, out="2470\n"):
    directory = tmp_path / name
    directory.mkdir()
    (directory / f"{name}.c").write_text(source)
    if out is not None:
        (directory / f"{name}.out").write_text(out)
    return directory


def test_a_program_runs_natively_only_with_main_an_expected_output_and_no_far_pointer(tmp_path):
    """DOS-only sources (far pointers) have no x86_64 reading: textfill and lru are not-applicable, not compile failures."""
    assert native.candidates(program(tmp_path)) is not None
    assert native.candidates(program(tmp_path, "g", out=None)) is None
    assert native.candidates(program(tmp_path, "h", "char far *p; int main(void) { return 0; }\n")) is None
    assert native.candidates(program(tmp_path, "i", "int f(void) { return 0; }\n")) is None


def test_a_row_counts_one_run_of_the_program_not_the_start_up_or_the_driver(tmp_path):
    """Instructions are (11 runs - 1 run) / 10 of `bench_main_`: a loop of n iterations must grow with n, and the build of the same source twice reads equal."""
    if shutil.which("gcc") is None or native.perf_missing():
        pytest.skip("needs gcc and perf")
    bench = tmp_path / "inc"
    bench.mkdir()
    one, two = program(tmp_path), program(tmp_path, "f2")
    small = native.measure_one("gcc64", tmp_path, one, one / "f.c", "O1", tmp_path / "w", bench)
    again = native.measure_one("gcc64", tmp_path, two, two / "f2.c", "O1", tmp_path / "w", bench)
    big_dir = program(tmp_path, "big", PROGRAM.replace("bench_f(20)", "bench_f(200)"), out="2646700\n")
    big = native.measure_one("gcc64", tmp_path, big_dir, big_dir / "big.c", "O1", tmp_path / "w", bench)
    assert "error" not in small and "error" not in big, (small, big)
    assert abs(small["instructions"] - again["instructions"]) <= 2
    assert big["instructions"] > 5 * small["instructions"]


def test_a_program_that_prints_the_wrong_value_is_an_error_not_a_count(tmp_path):
    if shutil.which("gcc") is None or native.perf_missing():
        pytest.skip("needs gcc and perf")
    directory = program(tmp_path, out="1\n")
    row = native.measure_one("gcc64", tmp_path, directory, directory / "f.c", "O1", tmp_path / "w", tmp_path)
    assert "prints" in row.get("error", "") and "instructions" not in row
