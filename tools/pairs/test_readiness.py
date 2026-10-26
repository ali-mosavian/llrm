"""The readiness table lists a program larger or slower than main, with its owner column blank, and counts them in the headline."""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import readiness  # noqa: E402


def test_growers_are_the_programs_larger_in_new_worst_first_and_an_uncompiled_one_is_not_compared():
    base = {"a.c": 100, "b.c": 100, "c.c": 100, "d.c": 0}
    new = {"a.c": 150, "b.c": 90, "c.c": 100, "d.c": 50}
    larger, count, was, now = readiness.growers(base, new)
    assert larger == [("a.c", 100, 150)] and (count, was, now) == (3, 300, 340)


def test_slower_counts_only_programs_measured_on_both_sides():
    base = {"rows": {"a": {"clocks": 100}, "b": {"clocks": 100}, "c": {"error": "x"}}}
    new = {"rows": {"a": {"clocks": 130}, "b": {"clocks": 90}, "c": {"clocks": 1}}}
    assert readiness.slower(base, new) == ([("a", 100, 130)], 2)


def test_the_headline_counts_and_every_row_has_an_empty_owner_column():
    sized = {"-Os": ([("quicksort.nib", 24, 39)], 2, 50, 70)}
    lines = readiness.report(["h"], sized, ([("fib.bas", 100, 110)], 5))
    assert "HEADLINE m16 -Os: 50 -> 70 B (+20); 1 of 2 programs larger" in lines
    assert any(line.startswith("HEADLINE m16 -Os clocks: 1 of 5 programs slower") for line in lines)
    rows = [line for line in lines if line.startswith("  ")]
    assert len(rows) == 2 and all(row.endswith("|") for row in rows) and "+15" in rows[0]


def test_a_table_is_measured_once_per_dist(tmp_path):
    calls = []
    path = tmp_path / "t.json"
    for _ in range(2):
        assert readiness.cached(path, lambda: calls.append(1) or {"a": 1}) == {"a": 1}
    assert len(calls) == 1


def test_cells_counts_the_distinct_bp_cells_of_one_function_only():
    text = "_sort proc near\n mov [bp-2], ax\n mov [bp-2], bx\n mov [bp-4], cx\n_sort endp\n_other proc near\n mov [bp-6], ax\n_other endp\n"
    assert readiness.cells(text, "_sort") == 2


def test_a_program_folded_on_main_is_marked_tiny_not_listed_as_a_12x_slowdown():
    """x87_arguments.c: main folded the whole program (237 clocks, the start-up), the tip keeps the loop like the pin (2893): the table read it as +1120%."""
    rows = {name: {"clocks": 1, "instructions": n, "language": ".c"} for name, n in {"floor.c": 100, "folded.c": 150, "real.c": 5000}.items()}
    assert readiness.tiny({"rows": rows}) == {"floor.c", "folded.c"}
    lines = readiness.report(["h"], {}, ([("folded.c", 237, 2893)], 3), {"folded.c"})
    assert any(line.startswith("  folded.c tiny") for line in lines) and "1 of them tiny" in " ".join(lines)
