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
