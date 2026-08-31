"""
qbopt/memory.py's own gate: the three classifications the analysis rests on,
and the corpus-wide invariants that catch a regression in any of them.
"""

from pathlib import Path

import pytest
from iced_x86 import Register

import corpus
from qbopt import memory
from qbopt.declen import run
from qbopt.module import Addr
from qbopt.module import Space
from qbopt.module import literal_only

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))
DGROUP = frozenset({1, 2, 3})


def only(code: bytes) -> memory.Access | memory.Unnameable | None:
    insns, _ = run(code, 0, len(code))
    return memory.access_of(insns[0], literal_only)


def test_a_push_of_an_immediate_names_no_memory_this_tracks() -> None:
    """pushd 0x40000 -- the stack cell is not a variable.

    Getting this wrong is what made every one of nbody's loops unanalysable,
    because push is around a third of the instructions in this corpus.
    """
    assert only(b"\x66\x68\x00\x00\x04\x00") is None


def test_a_push_of_a_static_names_the_static_it_reads() -> None:
    """push dword [0x1234] reads the static AND writes a stack cell; the
    static is the half worth tracking, and it is a read, not a write."""
    found = only(b"\x66\xff\x36\x34\x12")
    assert isinstance(found, memory.Access)
    assert found.reads and not found.writes


def test_an_unnameable_operand_is_not_silently_harmless() -> None:
    """add [bx+si],al -- lift.operand() refuses an index register, and this
    has to say UNKNOWN rather than None, which callers treat as harmless."""
    assert only(b"\x00\x00") is memory.UNKNOWN


def test_a_long_stored_as_two_words_is_read_back_as_one_dword() -> None:
    """The byte granularity is not a refinement -- BC has no dword store, so
    whole-access matching would call these unrelated."""
    low = memory.Access(Addr(Space.SEGMENT, 0x76, 5), 2, False, True)
    high = memory.Access(Addr(Space.SEGMENT, 0x78, 5), 2, False, True)
    whole = memory.Access(Addr(Space.SEGMENT, 0x76, 5), 4, True, False)
    covered = set(low.cells) | set(high.cells)
    assert set(whole.cells) <= covered


@pytest.mark.parametrize(
    ("cell_disp", "write_disp", "expected"),
    [(0x6, 0x6, True), (0x7, 0x6, True), (0xA, 0x6, False), (0x2, 0x6, False)],
)
def test_two_indexed_accesses_through_one_base_are_plain_arithmetic(
    cell_disp: int, write_disp: int, expected: bool
) -> None:
    """[si+6] and [si+10] differ by their displacements whatever si holds.

    module.may_alias refuses to say so, correctly -- it cannot know the base
    is unchanged. memory.py can, because it drops every cell whose base an
    instruction writes.
    """
    cell = Addr(Space.SEGMENT, cell_disp, 5, Register.SI)
    write = memory.Access(Addr(Space.SEGMENT, write_disp, 5, Register.SI), 4, False, True)
    assert memory.aliases(cell, write, DGROUP) is expected


def test_a_different_base_register_is_never_settled_by_arithmetic() -> None:
    """[si+6] and [di+6] can be the same byte; nothing here knows si <> di."""
    cell = Addr(Space.SEGMENT, 0x6, 5, Register.SI)
    write = memory.Access(Addr(Space.SEGMENT, 0x40, 5, Register.DI), 4, False, True)
    assert memory.aliases(cell, write, DGROUP) is True


def test_an_unknown_call_is_a_barrier_by_construction() -> None:
    assert memory.survives("B$MUI4") is True
    assert memory.survives("NOT_A_ROUTINE") is False
    assert memory.survives(None) is False
    assert memory.survives("B$EVCK") is False, "it can dispatch into user code"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_reported_load_is_really_a_load(obj: Path) -> None:
    """A redundant-load report that named a store would licence deleting one."""
    found = corpus.loaded(obj)
    assert found is not None
    partitioned = corpus.partitioned(obj)
    if not partitioned:
        return

    reported = memory.redundant_loads(partitioned, found.resolve, found.calls, found.dgroup)
    at = {insn.at: insn for block in partitioned for insn in block.insns}
    for addresses in reported.values():
        for one in addresses:
            access = memory.access_of(at[one], found.resolve)
            assert isinstance(access, memory.Access)
            assert access.reads and not access.writes


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_dead_store_is_a_store_and_never_a_call_site(obj: Path) -> None:
    found = corpus.loaded(obj)
    assert found is not None
    partitioned = corpus.partitioned(obj)
    if not partitioned:
        return

    reported = memory.dead_stores(partitioned, found.resolve, found.calls, found.dgroup)
    at = {insn.at: insn for block in partitioned for insn in block.insns}
    for addresses in reported.values():
        for one in addresses:
            assert one not in found.calls
            access = memory.access_of(at[one], found.resolve)
            assert isinstance(access, memory.Access)
            assert access.writes


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_availability_never_claims_a_cell_nothing_names(obj: Path) -> None:
    """Intersection at a join starts from a universe; a cell outside it would
    mean the top element leaked into an answer."""
    found = corpus.loaded(obj)
    assert found is not None
    partitioned = corpus.partitioned(obj)
    if not partitioned:
        return

    named: set[Addr] = set()
    for block in partitioned:
        for insn in block.insns:
            access = memory.access_of(insn, found.resolve)
            if isinstance(access, memory.Access):
                named.update(access.cells)

    incoming = memory.available(partitioned, found.resolve, found.calls, found.dgroup)
    for cells in incoming.values():
        assert cells <= named
