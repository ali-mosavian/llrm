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


def test_a_relocated_operand_is_refused_with_a_reason(obj: Path) -> None:
    # The widened instruction would be right -- its displacement field holds
    # zero, exactly as BC's does -- but it needs a FIXUPP of its own to say what
    # the zero stands for, and nothing writes records yet. Refused, and said so,
    # rather than emitted with a hardcoded address.
    _, found = rewrite(obj.read_bytes(), dry_run=False)
    assert all(not region.taken for region in found)
    assert all(region.reason for region in found), "a refusal always names itself"
