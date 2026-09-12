"""
What the object file tells the analysis layer that the loaded image could not.
"""

import re
import shutil
import subprocess
from pathlib import Path

import pytest

import corpus
from qbopt.objectfile import omf
from qbopt.analysis import regions
from qbopt.legacy.lift import lift
from qbopt.objectfile import module
from qbopt.legacy.lift import literal_only

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
    found = corpus.loaded(obj)
    assert found is not None
    reached = corpus.reached(obj)
    if isinstance(reached, str):
        return
    blind, _, _ = lift(found.code, found.start, found.end, literal_only, reached)
    assert [value for value in blind if value.mem and value.mem.space is module.Space.SEGMENT] == []


def test_the_fixups_are_what_make_a_static_pair_visible(fixtures: Path) -> None:
    found = corpus.loaded(fixtures / "arith-v-g3.obj")
    assert found is not None
    reached = corpus.reached(fixtures / "arith-v-g3.obj")
    assert not isinstance(reached, str)
    blind, _, _ = lift(found.code, found.start, found.end, literal_only, reached)
    seeing, _, _ = lift(found.code, found.start, found.end, found.resolve, reached)
    assert blind == []
    assert len(seeing) > 20, "a program of long arithmetic, and all of it on statics"


def test_the_fixups_make_the_pairs_visible(fixtures: Path) -> None:
    found = corpus.loaded(fixtures / WITH_PAIRS)
    assert found is not None
    reached = corpus.reached(fixtures / WITH_PAIRS)
    assert not isinstance(reached, str)
    values, _, _ = lift(found.code, found.start, found.end, found.resolve, reached)
    assert values, "with the fixups resolved there are pairs to lift"
    assert all(value.mem is None or value.mem.space is module.Space.SEGMENT for value in values)


def test_a_runtime_call_is_a_lookup_not_a_guess(operator_obj: Path) -> None:
    found = corpus.loaded(operator_obj)
    assert found is not None
    assert {"B$CPI4", "B$DVI4"} <= set(found.calls.values())
    for at, name in found.calls.items():
        assert found.code[at] == module.CALL_FAR, f"{name} is not reached by a far call"


def test_an_indirect_jump_has_findable_targets(fixtures: Path) -> None:
    # ON k GOTO L1, L2, L3 compiles to FF /4, whose target is not computable
    # from the instruction. In an object the labels are three consecutive
    # offset16 fixups into the module's own code segment.
    found = corpus.loaded(fixtures / WITH_PAIRS)
    assert found is not None
    assert sorted(found.targets) == [0x46, 0x52, 0x5E, 0xEA]
    assert all(target < found.end for target in found.targets)


def test_the_module_header_is_a_data_structure_not_code(obj: Path) -> None:
    # Every module opens with a header BC fills in: a name, then fields
    # relocated into other segments and the module's own entry point. The
    # measurement, on all five fixtures: those fixups stop at 0x20, and the
    # first operand that belongs to an instruction is at 0x31. What sits in the
    # gap is not established, so nothing here depends on where it ends.
    found = corpus.loaded(obj)
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
    found = corpus.loaded(obj)
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
        # a DGROUP segment too: the stack is the last thing in DGROUP and
        # grows down, so it reaches a named variable only by overflowing
        # into it. Assumed, not proven -- see regions' own note.
        (module.Addr(module.Space.FRAME, -4), module.Addr(module.Space.SEGMENT, 0, 1), False),
        # and a pushed argument is not a named variable either, which is what
        # let lngmix's loop hold anything invariant at all
        (module.Addr(module.Space.STACK, -2), module.Addr(module.Space.SEGMENT, 6, 1), False),
        (module.Addr(module.Space.SEGMENT, 6, 1), module.Addr(module.Space.STACK, -2), False),
        # a stack slot against a frame slot stays conservative: same region,
        # displacements against different registers
        (module.Addr(module.Space.STACK, -2), module.Addr(module.Space.FRAME, -4), True),
        # two distinct SEGDEFs are two distinct segments
        (module.Addr(module.Space.SEGMENT, 0, 1), module.Addr(module.Space.SEGMENT, 2, 9), False),
        # the same segment, far enough apart that no width could reach
        (module.Addr(module.Space.SEGMENT, 0, 1), module.Addr(module.Space.SEGMENT, 64, 1), False),
        # the same segment, overlapping at the default widest access
        (module.Addr(module.Space.SEGMENT, 0, 1), module.Addr(module.Space.SEGMENT, 2, 1), True),
        # two frame slots, overlapping only because the width is unstated
        (module.Addr(module.Space.FRAME, -4), module.Addr(module.Space.FRAME, -6), True),
        # an indexed operand reaches anywhere in its own segment
        (
            module.Addr(module.Space.SEGMENT, 0, 1, module.Register.SI),
            module.Addr(module.Space.SEGMENT, 64, 1),
            True,
        ),
        # a group-target address is never provably disjoint from anything
        (module.Addr(module.Space.FRAME, -4), module.Addr(module.Space.GROUP, 0, 1), True),
        # two es:[bx] accesses -- never provably disjoint here even when the
        # displacements do not meet, the same catch-all an si-indexed static
        # gets: proving them apart needs bx AND es both unchanged, which is
        # memory.aliases()'s own business, not this arithmetic-only layer's
        (
            module.Addr(module.Space.FAR, 0, base=module.Register.BX, segment=module.Register.ES),
            module.Addr(module.Space.FAR, 64, base=module.Register.BX, segment=module.Register.ES),
            True,
        ),
        # different segment registers, same bx -- still never provably disjoint
        (
            module.Addr(module.Space.FAR, 0, base=module.Register.BX, segment=module.Register.ES),
            module.Addr(module.Space.FAR, 0, base=module.Register.BX, segment=module.Register.SS),
            True,
        ),
        # a far pointer against an ordinary static: nothing here can rule it out
        (
            module.Addr(module.Space.FAR, 0, base=module.Register.BX, segment=module.Register.ES),
            module.Addr(module.Space.SEGMENT, 0, 9),
            True,
        ),
    ],
)
def test_may_alias(a: module.Addr, b: module.Addr, expected: bool) -> None:
    assert regions.addresses(a, module.WIDEST, b, module.WIDEST) is expected


@pytest.mark.parametrize(
    ("width", "expected"),
    [(2, False), (4, True)],
)
def test_may_alias_narrows_with_a_known_width(width: int, expected: bool) -> None:
    """Two adjacent frame slots meet or not depending on how wide the access is."""
    a = module.Addr(module.Space.FRAME, -4)
    b = module.Addr(module.Space.FRAME, -6)
    assert regions.addresses(a, width, b, width) is expected


def test_may_alias_over_states_an_unstated_width() -> None:
    """The default must never report disjoint where a real width could overlap."""
    a = module.Addr(module.Space.SEGMENT, 0, 1)
    b = module.Addr(module.Space.SEGMENT, module.WIDEST - 1, 1)
    assert regions.addresses(a, module.WIDEST, b, module.WIDEST) is True


def test_may_alias_is_conservative_about_the_unknown() -> None:
    assert regions.addresses(None, 2, module.Addr(module.Space.FRAME, -4), 2) is True
    assert regions.addresses(module.Addr(module.Space.SEGMENT, 0, 9), 2, None, 2) is True


@pytest.mark.skipif(shutil.which("ndisasm") is None, reason="ndisasm is not installed")
def test_every_value_starts_where_ndisasm_says_an_instruction_does(mapped_obj: Path) -> None:
    # Stronger than the hand-built case: real BC output, and the boundaries come
    # from a decoder this project did not write.
    found = corpus.loaded(mapped_obj)
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
    mapped = corpus.mapped(mapped_obj)
    assert not isinstance(mapped, str)
    reached = corpus.reached(mapped_obj)
    assert not isinstance(reached, str)
    values, _, _ = lift(found.code, found.start, found.end, found.resolve, reached)

    # ndisasm decodes straight through, so it cannot know an ON GOTO table is
    # data and everything after it is out of step. Up to the first one the two
    # agree, and that is the part worth checking.
    limit = min((lo for lo, _hi in mapped.tables), default=found.end)
    assert all(value.at in boundaries for value in values if value.at < limit)


def test_an_indexed_operand_is_bounded_by_the_next_thing_named_after_it() -> None:
    """`m(r * w + c)` does not reach `w`.

    regions takes an indexed operand to read or write its whole segment,
    which is sound and stops LICM dead: matrix's inner loop has nothing
    invariant in it because the store to the array is taken to reach every
    scalar beside it.

    The object's own layout bounds it. BC gives each variable a
    displacement and every non-indexed operand names one exactly, so an
    array beginning at 0x6 cannot run past the next thing named after it --
    0x328 in matrix, which is 802 bytes, and `DIM m(400)` to the byte.
    """
    from iced_x86 import Register

    from qbopt.objectfile import omf

    found = module.of(omf.parse(Path("fixtures/omf/matrix-p-g2.obj").read_bytes()))
    assert found is not None
    bounds = module.landmarks(found)
    seen = bounds[(module.Space.SEGMENT, 5)]
    assert seen[:2] == (0x0, 0x6) and 0x328 in seen, seen

    array = module.Addr(module.Space.SEGMENT, 0x6, 5, Register.SI)
    assert module.reach(array, 2, bounds) == (0x6, 0x328), "up to the next name, and no further"

    for disp in (0x328, 0x32A, 0x32C):
        scalar = module.Addr(module.Space.SEGMENT, disp, 5)
        assert regions.addresses(array, 2, scalar, 2), "unbounded, it reaches everything"
        assert not regions.addresses(array, 2, scalar, 2, bounds), f"bounded, it cannot reach {disp:#x}"

    # Inside the array it still may, which is what keeps this a bound and
    # not a licence.
    inside = module.Addr(module.Space.SEGMENT, 0x100, 5)
    assert regions.addresses(array, 2, inside, 2, bounds)

    # And a segment with no names to bound it by is unchanged.
    assert module.reach(array, 2, {}) is None
    assert regions.addresses(array, 2, module.Addr(module.Space.SEGMENT, 0x328, 5), 2, {})


def test_the_compiler_that_made_an_object_is_read_off_it() -> None:
    """B$ENRA reads bx under VBDOS and does not under PDS, so a contract
    keyed by name alone cannot be right for both.

    COMENT class 0x00 names the compiler in every object, and the library
    comment beside it names the runtime the routine actually came from --
    two fields agreeing, so the identity does not rest on one string.
    """
    from pathlib import Path

    from qbopt.objectfile import omf
    from qbopt.objectfile import module

    for name, want in (
        ("bools-q-O.obj", module.Family.QUICKBASIC),
        ("fpemu-p-evt.obj", module.Family.PDS),
        ("cmpord-v-g3.obj", module.Family.VBDOS),
    ):
        at = Path("fixtures/omf") / name
        assert at.exists(), f"{name} is checked in and this test needs it"
        got = module.family(omf.parse(at.read_bytes()))
        assert got is want, f"{name} reads as {got}"
