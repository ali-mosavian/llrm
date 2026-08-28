"""
What the object file tells the analysis layer that the loaded image could not.
"""

from pathlib import Path

import pytest

from qbopt import omf
from qbopt import module
from qbopt.lift import lift
from qbopt.lift import literal_only

# The operator fixtures are compare-and-divide programs: both are calls into the
# runtime, so they contain no instruction pair to lift. jumptable.obj is the one
# with pairs in it.
WITH_PAIRS = "jumptable.obj"


def test_the_lifter_is_blind_without_the_fixups(obj: Path) -> None:
    # In an object a static's address is not in the code -- the displacement
    # field holds zero and the offset is in the fixup. Read the code alone and
    # every operand looks like address zero, so a load pair's two halves are
    # both 0 and the hi == lo + 2 test can never hold.
    found = module.load(obj)
    assert found is not None
    blind, _ = lift(found.code, found.start, found.end, literal_only)
    assert blind == []


def test_the_fixups_make_the_pairs_visible(fixtures: Path) -> None:
    found = module.load(fixtures / WITH_PAIRS)
    assert found is not None
    values, _ = lift(found.code, found.start, found.end, found.resolve)
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
    # first operand that belongs to an instruction is at 0x32. What sits in the
    # gap is not established, so nothing here depends on where it ends.
    found = module.load(obj)
    assert found is not None
    header = [at for at in found.operands if at <= 0x20]
    assert header, "the header carries relocated fields"
    assert not [at for at in found.operands if 0x20 < at < 0x32], "and nothing between it and the first operand"


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
