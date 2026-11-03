"""`uv run --project tools python -m pytest tools/test_wasm_cells.py`

A comparator that flags only code bytes reports "0 worse" for a change that runs more instructions in the same bytes.
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
HEADER = "## -O2\nprogram llrm_B clang_B llrm_instr clang_instr result\n"


def run(tmp_path, base, new):
    (tmp_path / "base.txt").write_text(HEADER + base)
    (tmp_path / "new.txt").write_text(HEADER + new)
    return subprocess.run(
        [sys.executable, str(HERE / "wasm_cells.py"), str(tmp_path / "base.txt"), str(tmp_path / "new.txt")],
        capture_output=True,
        text=True,
    )


def test_a_cell_with_the_same_bytes_and_more_instructions_is_worse(tmp_path):
    out = run(tmp_path, "sieve 435 440 54723 54723 same\n", "sieve 435 440 56767 54723 same\n")
    assert out.returncode == 1
    assert "WORSE -O2 sieve bytes 435 -> 435, instrs 54723 -> 56767" in out.stdout


def test_a_cell_that_stops_building_is_broken(tmp_path):
    out = run(tmp_path, "sieve 435 440 54723 54723 same\n", "sieve llrm-cc FAIL: boom\n")
    assert out.returncode == 1
    assert "BROKEN -O2 sieve" in out.stdout


def test_equal_or_better_cells_pass(tmp_path):
    out = run(tmp_path, "sieve 435 440 54723 54723 same\n", "sieve 430 440 54000 54723 same\n")
    assert out.returncode == 0
    assert "compared 1 of 1 cells" in out.stdout
