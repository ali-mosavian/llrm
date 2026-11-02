"""`uv run --project tools python -m pytest tools/test_bench6502_cells.py`

The comparison once dropped a cell that went from numbers to `n/a (cl65: Symbol 'rc35' is undefined)`: lz4 and lz4c broke
and "181 cells" was 179 compared. A cell that stops building is a failure, not a missing row.
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
HEADER = "| kernel | ours |\n|---|---|\n"


def run(tmp_path, base, new):
    (tmp_path / "base.txt").write_text(HEADER + base)
    (tmp_path / "new.txt").write_text(HEADER + new)
    return subprocess.run(
        [sys.executable, str(HERE / "bench6502_cells.py"), str(tmp_path / "base.txt"), str(tmp_path / "new.txt")],
        capture_output=True,
        text=True,
    )


def test_a_cell_that_goes_from_numbers_to_na_fails_the_comparison(tmp_path):
    out = run(tmp_path, "| lz4 | 378 B, 37389 cy |\n| gcd | 47 B, 239 cy |\n", "| lz4 | n/a (cl65: Symbol 'rc35' is undefined) |\n| gcd | 47 B, 239 cy |\n")
    assert out.returncode == 1
    assert out.stdout.startswith("BROKEN lz4 ours")
    assert "compared 1 of 2 cells" in out.stdout


def test_cells_that_all_build_pass_and_the_worse_ones_are_listed(tmp_path):
    out = run(tmp_path, "| gcd | 47 B, 239 cy |\n", "| gcd | 49 B, 230 cy |\n")
    assert out.returncode == 0
    assert "worse (2, -9, 'gcd', 'ours')" in out.stdout
