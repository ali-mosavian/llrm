"""A pair whose new binary fails a program the base compiled is refused, and both failure counts head the table."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sizes  # noqa: E402


def test_a_program_the_new_binary_fails_and_the_base_compiled_breaks_the_premise():
    """lir-cleanup-port's pair read 'failed to compile' as noise: six BASIC programs the base compiled died in the new build."""
    header, broken = sizes.premise({"a": 100, "b": 200, "c": None}, {"a": 100, "b": None, "c": None})
    assert broken == ["b"]
    assert "failed in base: 1, failed in new: 2" in header and "new fails what base compiled: 1" in header


def test_a_failure_on_both_sides_is_counted_but_does_not_break_the_premise():
    header, broken = sizes.premise({"a": None, "b": 5}, {"a": None, "b": 5})
    assert broken == [] and header == "failed in base: 1, failed in new: 1"
