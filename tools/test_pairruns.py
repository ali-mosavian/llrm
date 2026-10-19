"""A rows pair names the tests/run outputs a flip changed."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pairruns  # noqa: E402

BASE = "[c] 350 programs: 350 pass, 0 known, 0 fail\nFAIL  examples/planets.nib: line 5: want 'a', got 'b'\n"


def test_a_flip_that_changes_one_output_is_reported_with_the_test_and_the_level():
    """JumpThreading on by default printed 30921 for 30192 in logical_value.c at -O2 and -Os (m16 and m32); the pair showed clocks and bytes only."""
    new = BASE + "FAIL  c/logical_value.c [-O2]: line 1: want '30192', got '30921'\nFAIL  c/logical_value.c [-Os] [x86-m32]: line 1: want '30192', got '30921'\n"
    added, fixed = pairruns.compare(BASE, new)
    assert added == [
        "NEW FAIL c/logical_value.c [-O2]: line 1: want '30192', got '30921'",
        "NEW FAIL c/logical_value.c [-Os] [x86-m32]: line 1: want '30192', got '30921'",
    ] and fixed == []


def test_a_failure_both_sides_have_is_not_the_flips_and_a_fixed_one_is_named():
    added, fixed = pairruns.compare(BASE, "[c] 350 programs: 350 pass, 0 known, 0 fail\n")
    assert added == [] and fixed == ["examples/planets.nib"]
    assert pairruns.compare(BASE, BASE) == ([], [])
