"""
The rewriter over the committed objects. No emulator, no compilers.

These are the invariants that stand between a change to the emitter and a
program that links, runs, and quietly computes the wrong thing.
"""

from pathlib import Path

import pytest

from qbopt import omf
from qbopt.rewrite import rewrite

pytestmark = pytest.mark.corpus


def test_a_dry_run_writes_the_input_back_unchanged(obj: Path) -> None:
    data = obj.read_bytes()
    out, _ = rewrite(data, dry_run=True)
    assert out == data


def test_a_dry_run_takes_no_region(obj: Path) -> None:
    _, found = rewrite(obj.read_bytes(), dry_run=True)
    assert [r for r in found if r.taken] == []


def test_the_records_survive_a_real_pass(obj: Path) -> None:
    data = obj.read_bytes()
    out, _ = rewrite(data, dry_run=False)
    assert [(r.type, r.body) for r in omf.parse(out)] == [(r.type, r.body) for r in omf.parse(data)]


def test_rewriting_the_output_finds_nothing_new(obj: Path) -> None:
    out, _ = rewrite(obj.read_bytes(), dry_run=False)
    _, again = rewrite(out, dry_run=False)
    assert [r for r in again if r.taken] == []


def test_no_region_fires_until_the_lifter_can_see_an_address(obj: Path) -> None:
    # In an object a static's address is not in the code: the operand is 0x0000
    # and the offset lives in the FIXUPP's target displacement, which the reader
    # discards. So lift's hi == lo + 2 pairing test is 0 == 2 and can never
    # succeed. This asserts the blocker rather than leaving a silent zero.
    _, found = rewrite(obj.read_bytes(), dry_run=False)
    assert found == []
