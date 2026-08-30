"""
What the object file tells the analysis layer that the loaded image could not.
"""

import re
import shutil
import subprocess
from pathlib import Path

import pytest

from qbopt import omf
from qbopt import module
from qbopt.lift import lift
from qbopt.blocks import code_map
from qbopt.lift import literal_only
from qbopt.blocks import instructions

# The operator fixtures are compare-and-divide programs: both are calls into the
# runtime, so they contain no instruction pair to lift. jumptable.obj is the one
# with pairs in it.
WITH_PAIRS = "jumptable.obj"


def test_a_static_operand_is_invisible_without_the_fixups(obj: Path) -> None:
    # A static's address is not in the code -- the displacement field holds zero
    # and the offset is in the fixup. Read the code alone and both halves of a
    # pair look like address zero, so hi == lo + 2 can never hold. bp-relative
    # operands are different: their displacement really is in the code, and they
    # pair with no fixup at all.
    found = module.load(obj)
    assert found is not None
    reached = instructions(found)
    if isinstance(reached, str):
        return
    blind, _, _ = lift(found.code, found.start, found.end, literal_only, reached)
    assert [value for value in blind if value.mem and value.mem.space is module.Space.SEGMENT] == []


def test_the_fixups_are_what_make_a_static_pair_visible(fixtures: Path) -> None:
    found = module.load(fixtures / "arith-v-g3.obj")
    assert found is not None
    reached = instructions(found)
    assert not isinstance(reached, str)
    blind, _, _ = lift(found.code, found.start, found.end, literal_only, reached)
    seeing, _, _ = lift(found.code, found.start, found.end, found.resolve, reached)
    assert blind == []
    assert len(seeing) > 20, "a program of long arithmetic, and all of it on statics"


def test_the_fixups_make_the_pairs_visible(fixtures: Path) -> None:
    found = module.load(fixtures / WITH_PAIRS)
    assert found is not None
    reached = instructions(found)
    assert not isinstance(reached, str)
    values, _, _ = lift(found.code, found.start, found.end, found.resolve, reached)
    assert values, "with the fixups resolved there are pairs to lift"
    assert all(value.mem is None or value.mem.space is module.Space.SEGMENT for value in values)


def test_a_runtime_call_is_a_lookup_not_a_guess(operator_obj: Path) -> None:
    found = module.load(operator_obj)
    assert found is not None
    assert {"B$CPI4", "B$DVI4"} <= set(found.calls.values())
    for at, name in found.calls.items():
        assert found.code[at] == module.CALL_FAR, f"{name} is not reached by a far call"


def test_an_indirect_jump_has_findable_targets(fixtures: Path) -> None:
    # ON k GOTO L1, L2, L3 compiles to FF /4, whose target is not computable
    # from the instruction. In an object the labels are three consecutive
    # offset16 fixups into the module's own code segment.
    found = module.load(fixtures / WITH_PAIRS)
    assert found is not None
    assert sorted(found.targets) == [0x46, 0x52, 0x5E, 0xEA]
    assert all(target < found.end for target in found.targets)


def test_the_module_header_is_a_data_structure_not_code(obj: Path) -> None:
    # Every module opens with a header BC fills in: a name, then fields
    # relocated into other segments and the module's own entry point. The
    # measurement, on all five fixtures: those fixups stop at 0x20, and the
    # first operand that belongs to an instruction is at 0x31. What sits in the
    # gap is not established, so nothing here depends on where it ends.
    found = module.load(obj)
    assert found is not None
    header = [at for at in found.operands if at <= 0x20]
    assert header, "the header carries relocated fields"
    assert not [at for at in found.operands if 0x20 < at < 0x31], "and nothing between it and the first operand"


@pytest.mark.parametrize(("at", "written"), [(0x50, 21), (0x5C, 9), (0x89, 80), (0xA7, 50), (0xC5, 20)])
def test_the_last_write_to_a_byte_is_the_one_that_counts(fixtures: Path, at: int, written: int) -> None:
    # BC emits overlapping LEDATA: short backpatch records arrive later in the
    # file at earlier offsets, filling in forward jump displacements. At each of
    # these offsets the first record wrote 0 and a later one wrote the value.
    records = omf.read(fixtures / WITH_PAIRS)
    found = omf.code_segment(records)
    assert found is not None
    seg, _name, size = found
    assert omf.segment_image(records, seg, size)[at] == written


def test_nothing_in_the_corpus_has_to_be_refused(obj: Path) -> None:
    assert omf.refusals(omf.read(obj)) == []


@pytest.mark.parametrize(
    ("kind", "why"),
    [
        (omf.LIDATA, "LIDATA"),
        (omf.FIXUPP + 1, "32-bit"),
        (0xC2, "COMDAT"),
    ],
)
def test_a_record_nothing_here_decodes_is_refused(fixtures: Path, kind: int, why: str) -> None:
    records = [*omf.read(fixtures / WITH_PAIRS), omf.Record(kind, b"\x00")]
    assert [reason for reason in omf.refusals(records) if why in reason]


def test_dgroup_is_populated_on_every_object(obj: Path) -> None:
    found = module.load(obj)
    assert found is not None
    assert len(found.dgroup) == 11
    assert found.seg not in found.dgroup, "the code segment is never in DGROUP"


DGROUP = frozenset({1, 2, 3})
OTHER = frozenset({9})


@pytest.mark.parametrize(
    ("a", "b", "expected"),
    [
        # a frame slot can never be the same byte as a segment DGROUP never lists
        (module.Addr(module.Space.FRAME, -4), module.Addr(module.Space.SEGMENT, 0, 9), False),
        (module.Addr(module.Space.SEGMENT, 0, 9), module.Addr(module.Space.FRAME, -4), False),
        # a DGROUP segment is the documented SS==DS assumption -- conservative True
        (module.Addr(module.Space.FRAME, -4), module.Addr(module.Space.SEGMENT, 0, 1), True),
        # two statics, or two frame slots, are not this predicate's business --
        # always conservative
        (module.Addr(module.Space.SEGMENT, 0, 1), module.Addr(module.Space.SEGMENT, 2, 9), True),
        (module.Addr(module.Space.FRAME, -4), module.Addr(module.Space.FRAME, -6), True),
        # a group-target address is never provably disjoint from anything
        (module.Addr(module.Space.FRAME, -4), module.Addr(module.Space.GROUP, 0, 1), True),
    ],
)
def test_may_alias(a: module.Addr, b: module.Addr, expected: bool) -> None:
    assert module.may_alias(a, b, DGROUP) is expected


def test_may_alias_is_conservative_about_the_unknown() -> None:
    assert module.may_alias(None, module.Addr(module.Space.FRAME, -4), DGROUP) is True
    assert module.may_alias(module.Addr(module.Space.SEGMENT, 0, 9), None, DGROUP) is True


@pytest.mark.skipif(shutil.which("ndisasm") is None, reason="ndisasm is not installed")
def test_every_value_starts_where_ndisasm_says_an_instruction_does(mapped_obj: Path) -> None:
    # Stronger than the hand-built case: real BC output, and the boundaries come
    # from a decoder this project did not write.
    found = module.load(mapped_obj)
    assert found is not None
    disassembled = subprocess.run(
        ["ndisasm", "-b16", "-o", str(found.start), "-"],
        input=found.code[found.start :],
        capture_output=True,
        check=True,
    )
    # a long instruction wraps its hex onto a continuation line, which carries
    # no offset of its own
    boundaries = {
        int(seen.group(1), 16)
        for line in disassembled.stdout.decode("latin1").splitlines()
        if (seen := re.match(r"^([0-9A-F]{8})  \S", line))
    }
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    reached = instructions(found)
    assert not isinstance(reached, str)
    values, _, _ = lift(found.code, found.start, found.end, found.resolve, reached)

    # ndisasm decodes straight through, so it cannot know an ON GOTO table is
    # data and everything after it is out of step. Up to the first one the two
    # agree, and that is the part worth checking.
    limit = min((lo for lo, _hi in mapped.tables), default=found.end)
    assert all(value.at in boundaries for value in values if value.at < limit)
