"""The per-pass table counts a pass by the instructions of the functions it ran over, and not a driver step that holds the passes."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pass_budgets  # noqa: E402

REPORT = """\
[counts] pass fold 4 runs 400 instructions
[counts] pass pipeline 1 runs 100 instructions
[counts] pass interprocedural 1 runs 0 instructions
[instr]       2.000 Minstr own        3.000 Minstr total        4x mir fold
[instr]       1.000 Minstr own       90.000 Minstr total        1x mir pipeline
[instr]       5.000 Minstr own        5.000 Minstr total        1x mir interprocedural
"""


def test_a_pass_is_priced_per_instruction_it_ran_over_and_a_container_step_is_not_a_pass():
    """`pipeline` held every pass of the rounds: its 90 M read as the worst pass by far (1.9 G at d_faces -O2)."""
    found = pass_budgets.parsed(REPORT)
    assert [one["pass"] for one in found] == ["fold"], found
    assert found[0]["per"] == 3e6 / 400
    assert found[0]["runs"] == 4


def test_the_passes_are_listed_by_the_instructions_they_spend_over_the_budget():
    rows = [
        {"pass": "cheap", "runs": 1, "ir": 1000, "total": 0.1, "own": 0.1, "per": 100.0},
        {"pass": "dear", "runs": 1, "ir": 1000, "total": 5.0, "own": 5.0, "per": 5000.0},
    ]
    lines = pass_budgets.table(rows, 300.0).splitlines()
    assert lines[1].startswith("dear") and lines[2].startswith("cheap")
