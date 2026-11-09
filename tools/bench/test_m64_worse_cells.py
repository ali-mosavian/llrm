import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import m64_worse_cells as worse_cells  # noqa: E402


def row(program, compiler, code, data=0):
    return f"x86-m64\t{program}\tO1\t{compiler}\t{code}\t0\t{data}\t\t\t\t\t\t\t\n"


def test_a_row_with_no_gcc_cell_is_refused_not_compared_with_clang():
    """z80's table took the next-best column for a missing reference and understated gaps by up to 1.7 kB."""
    text = row("p", "ours", 300) + row("p", "gcc", "FAIL") + row("p", "clang", 400)
    rows, missing = worse_cells.worse(worse_cells.cells(text.splitlines()))
    assert rows == [] and missing == ["p O1: no gcc"]


def test_a_complete_row_is_compared_with_the_best_of_the_two():
    text = row("p", "ours", 300) + row("p", "gcc", 250) + row("p", "clang", 280)
    rows, missing = worse_cells.worse(worse_cells.cells(text.splitlines()))
    assert rows == [("code", "p", "O1", 300, 250), ("code+data", "p", "O1", 300, 250)] and missing == []


def test_ours_winning_on_code_and_losing_on_code_plus_data_is_worse_on_the_second_axis():
    """The tables compared text only: a pool or table moved to .rodata made ours look smaller."""
    text = row("p", "ours", 200, 400) + row("p", "gcc", 250, 100) + row("p", "clang", 280, 120)
    rows, missing = worse_cells.worse(worse_cells.cells(text.splitlines()))
    assert rows == [("code+data", "p", "O1", 600, 350)] and missing == []


def test_the_data_of_an_object_counts_bss_and_a_const_table(tmp_path):
    """The matrix's data column was `size -A` .data + .rodata*: a 4 kB zero-filled buffer (histo) read 0 against 4608, grep 81 against 3137."""
    import shutil
    import subprocess

    if shutil.which("gcc") is None:
        import pytest

        pytest.skip("needs gcc")
    (tmp_path / "t.c").write_text("char bss[4096];\nconst int table[16] = {1,2,3};\nint f(int i) { bss[i] = 1; return bss[i + 1] + table[i & 15]; }\n")
    subprocess.run(["gcc", "-O1", "-fPIE", "-c", str(tmp_path / "t.c"), "-o", str(tmp_path / "t.o")], check=True)
    assert worse_cells.object_data(tmp_path / "t.o") >= 4096 + 64


def test_a_table_without_the_object_bytes_mark_is_refused(tmp_path):
    import subprocess
    import sys

    table = tmp_path / "old.tsv"
    table.write_text("# x86-m64 matrix, data_bytes .data+.rodata*\n" + row("p", "ours", 1) + row("p", "gcc", 1) + row("p", "clang", 1))
    done = subprocess.run([sys.executable, str(Path(worse_cells.__file__)), str(table)], capture_output=True, text=True)
    assert done.returncode != 0 and "data=object_bytes" in done.stderr
