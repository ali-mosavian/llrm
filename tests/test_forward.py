"""
qbopt/forward.py's own gate: a deletion is silent when it is wrong, so the
conditions that make one safe are the test.
"""

from pathlib import Path

import pytest

import corpus
from qbopt import memory
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
    found = corpus.loaded(obj)
    assert found is not None
    at = {insn.at: insn for block in corpus.partitioned(obj) for insn in block.insns}
    for one in removable(obj):
        access = memory.access_of(at[one], found.resolve)
        assert isinstance(access, memory.Access)
        assert access.reads and not access.writes
        assert forward._lands_in(at[one]) is not None


def test_a_call_clears_the_map_even_when_it_touches_no_caller_memory() -> None:
    """runtime.py proves B$MUI4 writes no caller memory at all, and it still
    returns with ax, cx, dx and bx changed.

    This map is "which register holds these bytes", so what survives a call
    is the memory, not the register holding a copy of it. Keeping an entry
    across one would forward a load from a register the callee overwrote.
    """
    from qbopt import runtime

    clean = runtime.contract("B$MUI4")
    assert clean.writes is runtime.Memory.NONE, "it touches no caller memory"
    assert clean.clobbers, "and still clobbers registers"
    assert memory.survives("B$MUI4") is True, "memory.py is asking about memory"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_removable_load_has_a_provider_with_no_call_between(obj: Path) -> None:
    """The property the clearing exists for, checked against the stream.

    Some earlier access in the same block names the same bytes, and no call
    sits between the two -- because a call returns with ax, cx, dx and bx
    changed whatever it did or did not do to memory. The provider is the
    nearest earlier access to that address, not the nearest access.
    """
    found = corpus.loaded(obj)
    assert found is not None
    for block in corpus.partitioned(obj):
        for one in sorted(x for x in removable(obj) if block.at <= x < block.end):
            mine = next(memory.access_of(insn, found.resolve) for insn in block.insns if insn.at == one)
            assert isinstance(mine, memory.Access)
            provider = None
            for insn in block.insns:
                if insn.at >= one:
                    break
                if insn.at in found.calls:
                    provider = None  # a call wipes what any register held
                    continue
                got = memory.access_of(insn, found.resolve)
                if isinstance(got, memory.Access) and got.addr == mine.addr and got.width == mine.width:
                    provider = insn.at
            assert provider is not None, f"{one:#06x} has no provider of its own bytes with no call between"


def test_an_accumulate_is_not_a_load() -> None:
    """`and cx,[x]` reads memory into cx and is not a load of it.

    iced reports cx as READ_WRITE there and WRITE in `mov cx,[x]`, which is
    the whole difference: the and combines the loaded bytes with what cx
    already held, so treating it as the load's provider throws the and
    away. arith-v-plain 0x123 and 0x127 are exactly this, and a pass that
    forwarded them emitted a program computing 0f0f0f0f where arith wants
    1f3f5f7f -- on nine of twelve real-compiler configurations, with the
    whole host suite green.

    removable() has never deleted one, because it fires only when the
    register already holds the loaded bytes and `cx = cx and cx` is cx.
    That is luck, not a rule: `cx = cx - cx` is zero.
    """

    found = corpus.loaded(Path("fixtures/omf/arith-v-plain.obj"))
    assert found is not None
    seen = 0
    for block in corpus.partitioned(Path("fixtures/omf/arith-v-plain.obj")):
        for insn in block.insns:
            if insn.at in (0x123, 0x127):
                seen += 1
                assert not forward._loads_only(insn), f"{insn.insn} read as a plain load"
                assert forward._lands_in(insn) is None
    assert seen == 2, "the two accumulate sites are gone from the fixture"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_removal_is_a_load_and_not_an_accumulate(obj: Path) -> None:
    """The rule, corpus-wide: nothing removable reads its own register."""
    hits = removable(obj)
    for block in corpus.partitioned(obj):
        for insn in block.insns:
            if insn.at in hits:
                assert forward._loads_only(insn), f"{obj.stem} {insn.at:#x}: {insn.insn}"
