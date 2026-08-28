"""
The shift map, and the branches that have no record to lean on.
"""

from pathlib import Path

import pytest

from qbopt import omf
from helpers import hx
from qbopt import module
from qbopt.declen import run
from qbopt.declen import decode
from qbopt.relocate import Edit
from qbopt.relocate import REL8
from qbopt.relocate import Shift
from qbopt.relocate import Branch
from qbopt.relocate import Inside
from qbopt.relocate import reaches
from qbopt.relocate import branches
from qbopt.relocate import relocate
from qbopt.relocate import retarget
from qbopt.blocks import instructions
from qbopt.relocate import retarget_branches

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
    found = module.load(mapped_obj)
    assert found is not None
    reached = instructions(found)
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
    found = module.load(mapped_obj)
    assert found is not None
    reached = instructions(found)
    assert not isinstance(reached, str), reached
    for branch in branches(found.code, reached):
        assert branch.width == (1 if found.code[branch.at] in REL8 else 2)


def relocated(obj: Path, shift: Shift) -> list[omf.Record] | str:
    records = omf.read(obj)
    found = omf.code_segment(records)
    assert found is not None
    seg, _name, size = found
    return relocate(records, seg, omf.segment_image(records, seg, size), shift)


def test_moving_nothing_reproduces_the_object_or_says_why_not(obj: Path) -> None:
    # The writer rebuilds every code LEDATA, re-emits every fixup and patches the
    # segment length, the publics and the line numbers. Asking it to move nothing
    # and getting the input back is the only guard that says it disturbs nothing.
    # Every object either does that or is refused -- there is no third outcome.
    out = relocated(obj, Shift.of([]))
    if isinstance(out, str):
        assert "no entry point" in out
    else:
        assert b"".join(record.emit() for record in out) == obj.read_bytes()


def test_most_of_the_corpus_can_be_moved(fixtures: Path) -> None:
    # A writer that refused everything would pass the invariant above.
    accepted = [p for p in sorted(fixtures.glob("*.obj")) if not isinstance(relocated(p, Shift.of([])), str)]
    assert len(accepted) > len(list(fixtures.glob("*.obj"))) // 2


def test_a_jump_table_in_the_code_segment_is_moved_not_refused(fixtures: Path) -> None:
    # ON GOTO puts its label words in the code segment. Reachability knows they
    # are a table rather than instructions, and each word is a fixup whose
    # target displacement moves with everything else.
    for name in ("jumps-v-g3.obj", "jumptable.obj"):
        out = relocated(fixtures / name, Shift.of([]))
        assert not isinstance(out, str), out


def test_a_region_that_is_not_whole_instructions_is_refused(fixtures: Path) -> None:
    out = relocated(fixtures / "arith-v-g3.obj", Shift.of([Edit(0x43, 0x45, b"\x90\x90")]))
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
