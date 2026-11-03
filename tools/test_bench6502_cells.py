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
    assert "BROKEN lz4 ours" in out.stdout.splitlines()[1]
    assert "compared 1 of 2 cells" in out.stdout


def test_cells_that_all_build_pass_and_the_worse_ones_are_listed(tmp_path):
    out = run(tmp_path, "| gcd | 47 B, 239 cy |\n", "| gcd | 49 B, 230 cy |\n")
    assert out.returncode == 0
    assert "worse (2, -9, 'gcd', 'ours')" in out.stdout


def test_two_legs_with_no_compiled_cell_fail_instead_of_comparing_zero_cells(tmp_path):
    """Both logs were `n/a (llrm-cc: No such file)` on every row: "compared 0 of 0 cells", exit 0, an A/B of nothing."""
    na = "| gcd | n/a (llrm-cc: No such file or directory) |\n| lz4 | n/a (llrm-cc: No such file or directory) |\n"
    out = run(tmp_path, na, na)
    assert out.returncode == 1
    assert "FAIL: a leg compiled no ours cell" in out.stdout
    assert "base: 0 of 2 ours cells compiled; new: 0 of 2" in out.stdout


def test_the_header_counts_compiled_cells_per_leg(tmp_path):
    out = run(tmp_path, "| gcd | 47 B, 239 cy |\n| lz4 | 378 B, 37389 cy |\n", "| gcd | 47 B, 239 cy |\n| lz4 | 378 B, 37389 cy |\n")
    assert out.returncode == 0
    assert out.stdout.startswith("base: 2 of 2 ours cells compiled; new: 2 of 2")


def test_legs_built_from_one_binary_fail_and_differing_binaries_pass(tmp_path):
    """llrm-cc is the same file in every leg (it calls the llc): only the llc tells the legs apart."""
    cell = "| gcd | 47 B, 239 cy |\n"
    (tmp_path / "a").write_bytes(b"one")
    (tmp_path / "b").write_bytes(b"two")
    (tmp_path / "base.txt").write_text(HEADER + cell)
    (tmp_path / "new.txt").write_text(HEADER + cell)

    def go(*pairs):
        flags = [x for pair in pairs for x in ("--bin", str(tmp_path / pair[0]), str(tmp_path / pair[1]))]
        return subprocess.run(
            [sys.executable, str(HERE / "bench6502_cells.py"), str(tmp_path / "base.txt"), str(tmp_path / "new.txt"), *flags],
            capture_output=True,
            text=True,
        )

    assert go(("a", "a")).returncode == 1
    assert go(("a", "a"), ("a", "b")).returncode == 0



def test_a_cell_with_the_same_bytes_and_more_clocks_is_listed_as_worse(tmp_path):
    """The unroll A/B once read "none worse" while the list covered bytes only; clocks went up unseen."""
    out = run(tmp_path, "| gcd | 47 B, 239 cy |\n", "| gcd | 47 B, 251 cy |\n")
    assert "worse (0, 12, 'gcd', 'ours')" in out.stdout


def test_a_cell_with_the_same_bytes_and_clocks_and_more_instructions_is_listed_as_worse(tmp_path):
    """Cells that carry instruction counts ("N in"): more instructions at equal bytes and clocks is worse."""
    out = run(tmp_path, "| gcd | 47 B, 239 cy, 100 in |\n", "| gcd | 47 B, 239 cy, 110 in |\n")
    assert "worse (0, 0, 'gcd', 'ours') instrs +10" in out.stdout
    assert "instrs: 110 (base 100) over 1 of 1 cells" in out.stdout


def test_cells_without_instruction_counts_print_na_not_equal(tmp_path):
    out = run(tmp_path, "| gcd | 47 B, 239 cy |\n", "| gcd | 47 B, 239 cy |\n")
    assert "instrs: n/a" in out.stdout
    assert "worse" not in out.stdout


def test_instructions_are_compared_only_where_both_legs_carry_them(tmp_path):
    out = run(tmp_path, "| a | 1 B, 2 cy, 5 in |\n| b | 1 B, 2 cy |\n", "| a | 1 B, 2 cy, 5 in |\n| b | 1 B, 2 cy, 9 in |\n")
    assert "instrs: 5 (base 5) over 1 of 2 cells" in out.stdout
