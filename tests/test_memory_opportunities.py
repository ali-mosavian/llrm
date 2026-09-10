"""BOOLS's emitted high-word overwrite disappeared from the redundancy counter."""

import sys
from collections import Counter
from pathlib import Path
from types import SimpleNamespace

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import opportunity
from qbopt.model import ir, mir
from qbopt.objectfile.module import Addr, Space


def access(at, offset, width, load=False, indirect=False):
    ref = mir.MemRef(Addr(Space.SEGMENT, offset, 5), width,
                     base=mir.Value(99, 0) if indirect else None)
    return mir.Op(at, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.LOAD if load else mir.Kind.STORE,
                  loads=(ref,) if load else (), stores=() if load else (ref,))


def measured(*ops):
    found = Counter()
    opportunity._through(mir.MirBlock(0, (), ops, ()), SimpleNamespace(calls={}, dgroup=frozenset({5})),
                         {}, set(), found)
    return found


def test_bools_partial_overwrite_is_not_zero_redundancy():
    """Re-raise combined x=-1/t=0 into a dword; t=2 overwrote its unread high half."""
    found = measured(access(0, 14, 4), access(1, 16, 2))
    assert found["partial overwrite of unread stored bytes"] == 1
    assert found["store over a store nothing read"] == 0


@pytest.mark.parametrize("read_offset,read_width", [(16, 2), (17, 1)])
def test_observed_store_is_not_reported_as_unread(read_offset, read_width):
    found = measured(access(0, 16, 2), access(1, read_offset, read_width, load=True), access(2, 16, 2))
    assert found["store over a store nothing read"] == 0


def test_wide_overwrite_counts_the_two_dead_word_stores():
    found = measured(access(0, 14, 2), access(1, 16, 2), access(2, 14, 4))
    assert found["store over a store nothing read"] == 2


def test_unknown_address_does_not_prove_a_redundant_access():
    found = measured(access(0, 16, 2), access(1, 16, 2, load=True, indirect=True), access(2, 16, 2))
    assert not found


def test_partial_overlap_without_containment_is_counted():
    found = measured(access(0, 14, 4), access(1, 16, 4))
    assert found["partial overwrite of unread stored bytes"] == 1


def test_overwrite_evidence_survives_a_block_boundary():
    module = SimpleNamespace(calls={}, dgroup=frozenset({5}))
    written, read = opportunity._through(mir.MirBlock(0, (), (access(0, 14, 4),), (10,)),
                                         module, {}, set(), None)
    found = Counter()
    opportunity._through(mir.MirBlock(10, (), (access(10, 16, 2),), ()), module, written, read, found)
    assert found["partial overwrite of unread stored bytes"] == 1


def test_call_clears_unread_store_evidence():
    call = mir.Op(1, ir.Operation.CALL, "call", (), (), kind=mir.Kind.CALL)
    module = SimpleNamespace(calls={1: "unknown"}, dgroup=frozenset({5}))
    found = Counter()
    opportunity._through(mir.MirBlock(0, (), (access(0, 16, 2), call, access(2, 16, 2)), ()),
                         module, {}, set(), found)
    assert not found
