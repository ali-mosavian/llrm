"""
qbopt/forward.py's own gate: a deletion is silent when it is wrong, so the
conditions that make one safe are the test.
"""

from pathlib import Path

import pytest

import corpus
from qbopt import forward

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def removable(obj: Path) -> frozenset[int]:
    found = corpus.loaded(obj)
    assert found is not None
    return forward.removable(corpus.partitioned(obj), found.resolve, found.calls, found.dgroup)


def test_a_register_written_in_between_stops_the_reload_being_redundant() -> None:
    """arith-q-O 0x136 reloads [x] into ax, which loaded it at 0x10d -- with
    a plain `mov ax,cx` at 0x12d in between.

    Nothing about that instruction touches memory, so a rule that only
    invalidates on a memory write never sees it and calls the reload
    redundant. Deleting it leaves ax holding cx. This was live for one
    measurement and accounted for 36 of 66 supposed removals.
    """
    assert 0x136 not in removable(Path("fixtures/omf/arith-q-O.obj"))


def test_a_reload_of_a_spill_nothing_disturbed_is_redundant() -> None:
    """procs-q-O 0x11d: ax is stored to [bp-0Eh] at 0x117, and the only
    instruction before the reload writes a different address out of dx."""
    assert 0x11D in removable(Path("fixtures/omf/procs-q-O.obj"))


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_removable_load_really_is_a_load_into_one_register(obj: Path) -> None:
    from qbopt import memory

    found = corpus.loaded(obj)
    assert found is not None
    at = {insn.at: insn for block in corpus.partitioned(obj) for insn in block.insns}
    for one in removable(obj):
        access = memory.access_of(at[one], found.resolve)
        assert isinstance(access, memory.Access)
        assert access.reads and not access.writes
        assert forward._lands_in(at[one]) is not None
