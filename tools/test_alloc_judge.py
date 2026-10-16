"""The allocator judge reads what the compiler prints and the bench writes, and reports what changed."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import alloc_judge  # noqa: E402


def test_a_trial_of_a_function_is_not_counted_as_a_second_function():
    """Only a function's first allocation says what its body left: the trials after it are other shapes of the same body."""
    stderr = ("[regalloc] _walk@3: 50 insns, 4 spilled, cost 31.7, 0 forced\n"
              "[regalloc]   trial Whole (splitting false): 4 spilled, cost 32.6, 0 forced\n"
              "[regalloc] _walk@3: 50 insns, 1 spilled, cost 9.0, 0 forced\n"
              "[regalloc] _main: 7 insns, 0 spilled, cost 7, 0 forced\n")
    assert alloc_judge.first_spills(stderr) == {"_walk@3": 4, "_main": 0}


def test_a_row_worse_by_more_than_half_a_percent_is_listed_and_the_geomean_covers_the_shared_rows():
    base = {"a/c/O2": {"clocks": 100}, "b/c/O2": {"clocks": 200}, "only/c/O2": {"clocks": 5}}
    new = {"a/c/O2": {"clocks": 110}, "b/c/O2": {"clocks": 200}}
    line = alloc_judge.summary(base, new)[0]
    assert "n=2" in line and "worse 1" in line and "a/c/O2" in line, line
