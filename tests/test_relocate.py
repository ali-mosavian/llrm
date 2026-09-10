"""
The shift map, and the branches that have no record to lean on.
"""

from pathlib import Path


def test_expanded_operation_can_cross_records_without_splitting_fixups():
    """NDMAX's 60-dimensional HARY expansion exceeded one LEDATA and was refused."""
    from qbopt.objectfile.relocate import _boundaries, LEDATA_LIMIT
    size = LEDATA_LIMIT * 3
    fields = ((LEDATA_LIMIT - 1, LEDATA_LIMIT + 3), (2 * LEDATA_LIMIT - 2, 2 * LEDATA_LIMIT + 4))
    cuts = _boundaries(bytes(size), {}, 0, fields)
    assert cuts[0] == 0 and cuts[-1] == size
    assert all(0 < right - left <= LEDATA_LIMIT for left, right in zip(cuts, cuts[1:]))
    assert not any(low < cut < high for cut in cuts for low, high in fields)

import pytest

import corpus
from qbopt.objectfile import omf
from helpers import hx
from qbopt.frontend.declen import run
from qbopt.frontend.declen import decode
from qbopt.objectfile.relocate import Edit
from qbopt.objectfile.relocate import REL8
from qbopt.objectfile.relocate import Shift
from qbopt.objectfile.relocate import apply
from qbopt.objectfile.relocate import Branch
from qbopt.objectfile.relocate import Inside
from qbopt.objectfile.relocate import reaches

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))
from qbopt.objectfile.relocate import branches
from qbopt.objectfile.relocate import relocate
from qbopt.objectfile.relocate import retarget
from qbopt.objectfile.relocate import retarget_branches

# 0x20..0x30 shrinks to 8 bytes, 0x40..0x50 to 4
SHRUNK = Shift.of([Edit(0x20, 0x30, bytes(8)), Edit(0x40, 0x50, bytes(4))])


@pytest.mark.parametrize(
    ("before", "after"),
    [
        (0x00, 0x00),  # before every edit
        (0x20, 0x20),  # the first byte of an edit does not move
        (0x30, 0x28),  # its end moves by what the edit saved
        (0x3F, 0x37),  # and so does everything between the two edits
        (0x40, 0x38),
        (0x50, 0x3C),  # past both, so both savings apply
        (0x60, 0x4C),
    ],
)
def test_where_an_offset_ends_up(before: int, after: int) -> None:
    assert SHRUNK.at(before) == after


@pytest.mark.parametrize("inside", [0x21, 0x28, 0x2F, 0x41, 0x4F])
def test_an_offset_inside_a_rewritten_region_is_an_error(inside: int) -> None:
    # The ON GOTO table points at arbitrary code offsets. One landing inside a
    # region means the region was chosen wrong, and clamping it would put the
    # jump somewhere plausible and wrong.
    with pytest.raises(Inside):
        SHRUNK.at(inside)


def test_edits_may_not_overlap() -> None:
    with pytest.raises(ValueError, match="overlap"):
        Shift.of([Edit(0x10, 0x20, bytes(4)), Edit(0x18, 0x28, bytes(4))])


def test_a_branch_is_read_relative_to_the_instruction_after_it() -> None:
    #  0: EB 05        jmp +5   -> 0x07
    #  2: 74 FE        jz  -2   -> 0x02
    #  4: E9 03 00     jmp +3   -> 0x0a
    code = bytes.fromhex("EB05 74FE E90300")
    decoded, gave_up = run(code, 0, len(code))
    assert gave_up is None
    assert [(b.at, b.target) for b in branches(code, decoded)] == [(0, 7), (2, 2), (4, 10)]


def test_a_branch_is_retargeted_from_its_own_end_not_its_start() -> None:
    # A branch at 0x10 over a region that shrinks by 8: the target moves, the
    # branch does not. Mapping from `at` instead of `end` is off by its length.
    branch = Branch(at=0x10, end=0x12, field_at=0x11, width=1, target=0x30)
    shift = Shift.of([Edit(0x20, 0x30, bytes(8))])
    assert retarget(branch, shift) == 0x28 - 0x12


def test_a_branch_ending_exactly_at_a_pure_insertion_is_not_shifted_past_it() -> None:
    # EB 05 = jmp, ends at 2, targets the CC at 7. A zero-width edit at 2 -- a
    # pure insertion, nothing removed -- does not move the branch: its bytes
    # (0, 1) are copied verbatim by apply(). But the physical byte right after
    # it, in the *new* image, is the inserted one, not whatever the edit's own
    # delta would place there -- so the branch's own end must not be pushed
    # past the insertion the way a target landing on that same offset should.
    code = hx("EB05") + bytes(5) + hx("CC")
    branch = Branch(at=0, end=2, field_at=1, width=1, target=7)
    shift = Shift.of([Edit(2, 2, b"\x90")])
    moved = apply(code, shift)
    assert moved == hx("EB05") + b"\x90" + bytes(5) + hx("CC")
    assert retarget(branch, shift) == moved.index(0xCC) - 2


def test_a_rel8_that_no_longer_reaches_is_refused_not_truncated() -> None:
    # Truncating wraps mod 256 and lands mid-instruction: no trap, no LINK
    # diagnostic, and a program that runs garbage down a path that may be rare.
    branch = Branch(at=0x00, end=0x02, field_at=0x01, width=1, target=0x70)
    assert reaches(branch, retarget(branch, Shift.of([Edit(0x10, 0x20, bytes(0x20))])))
    assert not reaches(branch, retarget(branch, Shift.of([Edit(0x10, 0x20, bytes(0x80))])))


def test_a_shrinking_shift_can_never_push_a_rel8_out_of_range(mapped_obj: Path) -> None:
    # Every displacement's magnitude can only fall when the code between the
    # branch and its target shrinks, which is what makes shrink-only motion safe
    # without a relaxation pass.
    found = corpus.loaded(mapped_obj)
    assert found is not None
    reached = corpus.reached(mapped_obj)
    assert not isinstance(reached, str), reached
    reachable = branches(found.code, reached)
    shift = Shift.of([Edit(at, at + 8, bytes(4)) for at in range(0x40, found.end - 8, 0x20)])
    for branch in reachable:
        if branch.width != 1:
            continue
        try:
            moved = retarget(branch, shift)
        except Inside:
            continue
        assert abs(moved) <= abs(branch.target - branch.end)


def test_the_rel8_opcodes_are_the_ones_that_take_a_byte(mapped_obj: Path) -> None:
    found = corpus.loaded(mapped_obj)
    assert found is not None
    reached = corpus.reached(mapped_obj)
    assert not isinstance(reached, str), reached
    for branch in branches(found.code, reached):
        assert branch.width == (1 if found.code[branch.at] in REL8 else 2)


def test_moving_nothing_reproduces_the_object_or_says_why_not(obj: Path) -> None:
    # The writer rebuilds every code LEDATA, re-emits every fixup and patches the
    # segment length, the publics and the line numbers. Asking it to move nothing
    # and getting the input back is the only guard that says it disturbs nothing.
    # Every object either does that or is refused -- there is no third outcome.
    out = corpus.relocated(obj, Shift.of([]))
    if isinstance(out, str):
        assert "no entry point" in out
    else:
        assert b"".join(record.emit() for record in out) == obj.read_bytes()


def test_most_of_the_corpus_can_be_moved(fixtures: Path) -> None:
    # A writer that refused everything would pass the invariant above.
    accepted = [p for p in sorted(fixtures.glob("*.obj")) if not isinstance(corpus.relocated(p, Shift.of([])), str)]
    assert len(accepted) > len(list(fixtures.glob("*.obj"))) // 2


def test_a_jump_table_in_the_code_segment_is_moved_not_refused(fixtures: Path) -> None:
    # ON GOTO puts its label words in the code segment. Reachability knows they
    # are a table rather than instructions, and each word is a fixup whose
    # target displacement moves with everything else.
    for name in ("jumps-v-g3.obj", "jumptable.obj"):
        out = corpus.relocated(fixtures / name, Shift.of([]))
        assert not isinstance(out, str), out


def test_a_region_that_is_not_whole_instructions_is_refused(fixtures: Path) -> None:
    out = corpus.relocated(fixtures / "arith-v-g3.obj", Shift.of([Edit(0x43, 0x45, b"\x90\x90")]))
    assert isinstance(out, str)
    assert "instruction" in out


def test_the_header_ends_before_the_first_instruction_operand(obj: Path) -> None:
    # What bounds the search for where code starts. Measured over the whole
    # corpus: header fields carry fixups up to 0x20, and the earliest operand
    # belonging to an instruction is at 0x31 -- in the /V /W builds, where the
    # event-polling call sits ahead of everything else.
    records = omf.read(obj)
    found = omf.code_segment(records)
    assert found is not None
    sites = sorted(fixup.offset for fixup in omf.fixups(records) if fixup.seg == found[0])
    assert not [at for at in sites if 0x20 < at < 0x31]


def labels(records: list[omf.Record], seg: int) -> list[int]:
    """Every offset in this segment that a fixup points at."""
    return sorted(
        fixup.disp
        for fixup in omf.fixups(records)
        if fixup.target == "segment" and fixup.index == seg and fixup.disp_pos is not None
    )


def test_moving_code_moves_what_the_jump_table_points_at(fixtures: Path) -> None:
    # The label words are fixups, so each has an offset that moves and a target
    # displacement that moves too. Shifting one without the other links cleanly
    # and jumps to the wrong statement, which is the whole class of bug the
    # relocation list exists for.
    records = omf.read(fixtures / "jumptable.obj")
    found = omf.code_segment(records)
    assert found is not None
    seg, _name, size = found
    assert labels(records, seg) == [0x46, 0x52, 0x5E, 0xEA]

    # one byte in at 0x36, which is an instruction boundary ahead of every label
    out = relocate(records, seg, omf.segment_image(records, seg, size), Shift.of([Edit(0x36, 0x36, b"\x90")]))
    assert not isinstance(out, str), out
    moved = omf.parse(b"".join(record.emit() for record in out))
    assert labels(moved, seg) == [0x47, 0x53, 0x5F, 0xEB]


def test_a_branch_that_would_no_longer_reach_refuses_the_whole_segment() -> None:
    # Truncating wraps mod 256 into the middle of an instruction: no trap, no
    # diagnostic from LINK, and a program that runs garbage down a rare path.
    code = hx("EB 7E") + bytes(0x7E) + hx("90")
    branch = decode(code, 0)
    assert branch is not None
    assert isinstance(retarget_branches(code, [branch], Shift.of([])), bytes)

    # grow what the branch jumps over until the displacement will not fit
    grown = Shift.of([Edit(0x10, 0x20, bytes(0x90))])
    assert isinstance(retarget_branches(code, [branch], grown), str)


def test_a_fixup_naming_an_offset_the_layout_did_not_place_is_refused(monkeypatch: pytest.MonkeyPatch) -> None:
    """omf.reemit leaves a field alone when it is given None.

    So a same-segment fixup whose displacement names an offset the layout
    could not place would keep the displacement it arrived with -- pointing
    at wherever that offset used to be, which is wrong the moment anything
    ahead of it changed length. Silent, and the kind of silence a relocation
    bug is: nothing crashes, an address is simply off.

    A *record* naming an unplaceable offset has always been refused. This is
    the same refusal for a fixup, and it stopped being unreachable when
    layout.py began carrying gaps reachability never walked into: a fixup
    could now name the inside of one.

    Forced here, since no object in the corpus produces it: one placement is
    removed from the map after the records have had theirs, so the fixup
    loop is the only thing that can see it missing.
    """
    from qbopt.objectfile import relocate
    from qbopt.wholeseg import REBUILT
    from qbopt.wholeseg import rebuilt

    real = relocate._mapped
    seen: list[int] = []

    def watch(offset: int, kept: int, moved: dict[int, int]) -> int | None:
        seen.append(offset)
        return real(offset, kept, moved)

    from qbopt.objectfile import module

    def asks_a_fixup(one: Path) -> bool:
        records = omf.parse(one.read_bytes())
        found = module.of(records)
        return found is not None and any(
            fixup.seg == found.seg
            and fixup.target == "segment"
            and fixup.index == found.seg
            and fixup.disp_pos is not None
            for fixup in omf.fixups(records)
        )

    monkeypatch.setattr(relocate, "_mapped", watch)
    obj = None
    for one in FIXTURES:
        seen.clear()
        if asks_a_fixup(one) and rebuilt(one.read_bytes())[1] == REBUILT and seen:
            obj = one
            break
    assert obj is not None, "no fixture rebuilds while a fixup names a code offset"
    # The records ask first, so anything asked for after the last of
    # them is a fixup's own displacement.
    # By call, not by value: the records ask about some of the same
    # offsets, and failing one of those trips their own refusal first,
    # which would leave this passing whatever the fixup loop did.
    last = len(seen) - 1
    calls = 0

    def missing(offset: int, kept: int, moved: dict[int, int]) -> int | None:
        nonlocal calls
        calls += 1
        return None if calls - 1 == last else real(offset, kept, moved)

    monkeypatch.setattr(relocate, "_mapped", missing)
    why = rebuilt(obj.read_bytes())[1]
    assert why != REBUILT, "a fixup naming an unplaceable offset was written anyway"
    assert "a fixup names" in why, why


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_rebuilt_object_maps_every_code_offset_it_names(obj: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """The invariant behind that refusal, over the whole corpus.

    Every offset a record or a fixup names in the rebuilt segment resolves
    to an instruction the layout placed. If one did not, the refusal above
    would fire -- so a rebuild that succeeds is the assertion.
    """
    from qbopt.objectfile import relocate
    from qbopt.wholeseg import REBUILT
    from qbopt.wholeseg import rebuilt

    unmapped: list[int] = []
    real = relocate._mapped

    def watch(offset: int, kept: int, moved: dict[int, int]) -> int | None:
        out = real(offset, kept, moved)
        if out is None:
            unmapped.append(offset)
        return out

    monkeypatch.setattr(relocate, "_mapped", watch)
    why = rebuilt(obj.read_bytes())[1]
    if why == REBUILT:
        assert not unmapped, f"{obj.stem}: rebuilt while {len(unmapped)} offsets did not map"
