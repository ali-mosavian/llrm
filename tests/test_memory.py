"""
qbopt/memory.py's own gate: the three classifications the analysis rests on,
and the corpus-wide invariants that catch a regression in any of them.
"""

from pathlib import Path

import pytest
from iced_x86 import Register

import corpus
from qbopt import ir
from qbopt import memory
from qbopt.declen import run
from qbopt.blocks import Ends
from qbopt.module import Addr
from qbopt.blocks import Block
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


@pytest.mark.parametrize(
    ("cell_disp", "write_disp", "expected"),
    [(0x6, 0x6, True), (0x7, 0x6, True), (0xA, 0x6, False), (0x2, 0x6, False)],
)
def test_two_far_accesses_through_one_bx_are_plain_arithmetic_too(
    cell_disp: int, write_disp: int, expected: bool
) -> None:
    """es:[bx+6] and es:[bx+10] differ by their displacements exactly as two
    si-indexed statics do -- the same argument, one register up, sound only
    while both bx AND es are unchanged between them."""
    cell = Addr(Space.FAR, cell_disp, base=Register.BX, segment=Register.ES)
    write = memory.Access(Addr(Space.FAR, write_disp, base=Register.BX, segment=Register.ES), 4, False, True)
    assert memory.aliases(cell, write, DGROUP) is expected


def test_a_different_segment_register_is_never_settled_by_arithmetic() -> None:
    """es:[bx+6] and ss:[bx+6] can be the same byte; nothing here knows es <> ss,
    the segment-register counterpart of the si/di case above."""
    cell = Addr(Space.FAR, 0x6, base=Register.BX, segment=Register.ES)
    write = memory.Access(Addr(Space.FAR, 0x6, base=Register.BX, segment=Register.SS), 4, False, True)
    assert memory.aliases(cell, write, DGROUP) is True


def test_reloading_the_segment_register_drops_a_tracked_far_cell() -> None:
    """`mov ax,es:[bx]` / `mov es,[si+2]` / `mov dx,es:[bx]` -- the middle
    instruction reloads es, so the third load may not be reported redundant
    even though its own address, read alone, matches the first load's."""
    reload_es = "26 8B 07" + "8E 44 02" + "26 8B 17"  # mov ax,es:[bx] / mov es,[si+2] / mov dx,es:[bx]
    no_reload = "26 8B 07" + "26 8B 17"  # mov ax,es:[bx] / mov dx,es:[bx], back to back

    def redundant_at(hex_bytes: str) -> tuple[int, ...]:
        code = bytes.fromhex(hex_bytes.replace(" ", ""))
        insns, tail = run(code, 0, len(code))
        assert tail is None, "every byte in this hand-built sequence decodes"
        block = Block(0, len(code), tuple(insns), Ends.FALLS_THROUGH, ())
        return memory.redundant_loads([block], literal_only, {}, frozenset())[0]

    assert redundant_at(no_reload) == (3,), "the second es:[bx] load is redundant when nothing reloads es"
    assert redundant_at(reload_es) == (), "reloading es must clear that cell's own availability"


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


def test_a_barrier_makes_frame_slots_live_not_only_statics() -> None:
    """A barrier may read any memory, and a frame slot is memory.

    Resetting liveness to the statics alone drops every live frame cell, so
    a store to one before a barrier reads as dead. bench/nbody.bas's
    PITSNAP stores the PIT reading to [bp-16h] at 0x438 and reads it back
    at 0x4ec with in/out barriers between; the store was reported dead, and
    deleting it would have corrupted the timer with nothing in the report
    to say so. 92 of 135 reported dead stores were this.
    """
    found = corpus.loaded(Path("fixtures/omf/procs-v-g3.obj"))
    assert found is not None
    partitioned = corpus.partitioned(Path("fixtures/omf/procs-v-g3.obj"))
    everything = memory.every_cell(partitioned, found.resolve)
    statics = memory.statics_of(partitioned, found.resolve)
    assert statics <= everything
    frames = {one for one in everything if one.space is Space.FRAME}
    assert frames, "procs has locals"
    assert not (frames & statics), "a frame slot is not a static"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_dead_store_is_overwritten_before_anything_reads_it(obj: Path) -> None:
    """The property the deletion rests on, checked against the instruction
    stream rather than against the analysis that produced it."""
    found = corpus.loaded(obj)
    assert found is not None
    partitioned = corpus.partitioned(obj)
    at = {insn.at: insn for block in partitioned for insn in block.insns}
    reported = memory.dead_stores(partitioned, found.resolve, found.calls, found.dgroup)
    for block in partitioned:
        for one in reported.get(block.at, ()):
            access = memory.access_of(at[one], found.resolve)
            assert isinstance(access, memory.Access)
            assert access.writes and not access.reads
            # nothing between it and the end of its own block may read those
            # bytes without something overwriting them first
            seen = False
            for insn in block.insns:
                if insn.at <= one:
                    continue
                other = memory.access_of(insn, found.resolve)
                if not isinstance(other, memory.Access):
                    continue
                if other.reads and set(other.cells) & set(access.cells):
                    assert seen, f"{one:#06x} is read at {insn.at:#06x} before being overwritten"
                if other.writes and set(access.cells) <= set(other.cells):
                    seen = True


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_store_through_a_recomputed_index_is_not_an_overwrite(obj: Path) -> None:
    """Two stores sharing an Addr are not two stores to one address.

    An Addr names the register, not its value, so six writes through
    `es:[bx]` -- six elements of one array with bx recomputed between --
    look identical. Absent from the live set means "overwritten before
    anything read it", so failing to notice makes the first five dead.

    Conservatism runs opposite to availability here, which is the part that
    is easy to get backwards and was: _forward drops a cell whose base
    changed, _backward has to make it live again. Dropping it there asserts
    a store is dead. arrprm printed 0 where it had stored 7 on nine of
    twelve configurations.
    """
    found = corpus.loaded(obj)
    assert found is not None
    partitioned = corpus.partitioned(obj)
    at = {insn.at: insn for block in partitioned for insn in block.insns}
    reported = memory.dead_stores(partitioned, found.resolve, found.calls, found.dgroup)
    for block in partitioned:
        prepared = memory.prepare(block, found.resolve, found.calls)
        clobbered_after = {}
        seen: set[int] = set()
        for step in reversed(prepared):
            clobbered_after[step.at] = frozenset(seen)
            seen |= set(step.clobbers)
        for one in reported.get(block.at, ()):
            access = memory.access_of(at[one], found.resolve)
            assert isinstance(access, memory.Access)
            if access.addr is None or access.addr.base == Register.NONE:
                continue
            root = ir.ROOT.get(access.addr.base, access.addr.base)
            assert root not in clobbered_after[one], (
                f"{one:#06x} is indexed by a register rewritten before the store that supposedly overwrites it"
            )
