"""
The OMF reader, against objects BC actually produced.

The fixtures are real BC output, one per configuration that emits a different
shape, plus one module with an ON GOTO and a SELECT CASE because that is where
the intra-segment references live.

The round trip is the test that matters: read a file, write it back, and
require the bytes to be identical. A pass that means to move code has to be
trusted not to disturb what it is not changing, and nothing else here is worth
anything until that holds.
"""

from pathlib import Path

import struct

import pytest

from qbopt import omf


def test_round_trip_is_byte_identical(obj: Path) -> None:
    data = obj.read_bytes()
    assert b"".join(r.emit() for r in omf.parse(data)) == data


def test_decodes_more_than_a_handful_of_records(obj: Path) -> None:
    assert len(omf.parse(obj.read_bytes())) > 10


def test_module_code_segment_is_found(obj: Path) -> None:
    segs = omf.segments(omf.read(obj))
    code = [s for s in segs[1:] if s and s[0].endswith("_CODE")]
    assert code, "no _CODE segment"
    assert code[0][1] > 0


def test_runtime_call_is_named_not_guessed_at(operator_obj: Path) -> None:
    # B$CPI4 is the long compare, and every configuration reaches it through an
    # EXTDEF -- the whole reason this is easier here than at run time.
    recs = omf.read(operator_obj)
    exts = omf.externals(recs)
    named = [
        omf.LOCNAME.get(x.loc, x.loc) for x in omf.fixups(recs) if x.target == "external" and exts[x.index] == "B$CPI4"
    ]
    assert named == ["ptr16:16"]


def test_groups_parses_dgroup_the_same_way_everywhere(obj: Path) -> None:
    # Measured: one GRPDEF per object, named DGROUP, the same eleven segments
    # in all 110 fixtures -- BC_CN, BC_DATA, BC_DS, BC_FT, BC_SA, BC_SAB,
    # BR_DATA, BR_SKYS, COMMON, ENMALLOC, NMALLOC.
    records = omf.read(obj)
    segs = omf.segments(records)
    groups = omf.groups(records)
    assert set(groups) == {"DGROUP"}
    named = [segs[i] for i in groups["DGROUP"]]
    assert all(named), "every DGROUP member is a real segment"
    names = sorted(segment[0] for segment in named if segment is not None)
    assert names == [
        "BC_CN",
        "BC_DATA",
        "BC_DS",
        "BC_FT",
        "BC_SA",
        "BC_SAB",
        "BR_DATA",
        "BR_SKYS",
        "COMMON",
        "ENMALLOC",
        "NMALLOC",
    ]


def test_threads_are_resolved(jumptable: list[omf.Record]) -> None:
    # BC leans on THREAD subrecords: 34 of these 40 fixups name a thread rather
    # than their target. Anything that moves code has to resolve them or it
    # cannot see most of the relocations at all.
    fx = omf.fixups(jumptable)
    assert len(fx) == 40
    assert not [x for x in fx if x.target == "thread"]


def test_on_goto_table_is_relocated_so_code_may_be_moved(jumptable: list[omf.Record]) -> None:
    # The three labels of "ON k GOTO L1, L2, L3" appear as three consecutive
    # offset16 fixups into the module's own code segment. That they are fixups
    # at all is what makes moving code tractable: BC does not bake intra-segment
    # offsets into the code where a rewriter could not see them.
    segs = omf.segments(jumptable)
    code_i = next(i for i, s in enumerate(segs) if s and s[0] == "JT_CODE")
    table = sorted(
        x.offset
        for x in omf.fixups(jumptable)
        if x.target == "segment" and x.index == code_i and x.loc == omf.LOC_OFF16
    )
    assert table == [0x0A, 0x40, 0x42, 0x44]


def test_a_fixup_carries_the_address_it_relocates(jumptable: list[omf.Record]) -> None:
    # In an object a label's address is not in the code -- the bytes there are
    # 0000 and the real offset is the fixup's target displacement. Discard it
    # and the lifter sees every operand as zero.
    segs = omf.segments(jumptable)
    code = next(i for i, s in enumerate(segs) if s and s[0] == "JT_CODE")
    into_code = [
        f for f in omf.fixups(jumptable) if f.target == "segment" and f.index == code and f.loc == omf.LOC_OFF16
    ]
    assert sorted(f.disp for f in into_code) == [0x46, 0x52, 0x5E, 0xEA]


def test_a_frame_thread_that_carries_no_index_is_not_read_as_one() -> None:
    # Frame methods 4 and 5 -- the location's segment, and the target's frame --
    # take no index. Testing method & 3 says otherwise for both, and eats a byte
    # that is not there. BC only emits method 1, so no fixture catches it.
    thread = 0x40 | (5 << 2)  # frame thread 0, method 5
    fixup = bytes([0xC4, 0x10, 0x80, 0x01, 0x34, 0x12])  # offset16 at 0x10, disp 0x1234
    found = omf.fixups([omf.Record(omf.FIXUPP, bytes([thread]) + fixup)])
    assert len(found) == 1
    assert (found[0].offset, found[0].loc, found[0].disp) == (0x10, omf.LOC_OFF16, 0x1234)


def test_every_byte_of_a_fixupp_is_accounted_for(obj: Path) -> None:
    # A decoder that desynchronises still returns fixups -- they are just
    # nonsense. The extents tiling the record exactly is what says otherwise.
    # Measured: this does not catch the frame-thread bug, because no fixture
    # uses frame method 4 or 5; that one needs the synthetic case above.
    records = omf.read(obj)
    by_record: dict[int, list[omf.Fixup]] = {}
    for fixup in omf.fixups(records):
        by_record.setdefault(id(fixup.record), []).append(fixup)

    for found in by_record.values():
        body = found[0].record.body
        assert found[-1].hi == len(body), "the last subrecord must end the record"
        for earlier, later in zip(found, found[1:], strict=False):
            assert earlier.hi <= later.lo, "subrecords must not overlap"
        for fixup in found:
            assert fixup.raw == body[fixup.lo : fixup.hi]


def _thread(is_frame: bool, number: int, method: int, index: int | None) -> bytes:
    lead = (0x40 if is_frame else 0) | (method << 2) | number
    return bytes([lead]) if index is None else bytes([lead]) + _as_index(index)


def _as_index(value: int) -> bytes:
    return bytes([value]) if value < 128 else bytes([0x80 | (value >> 8), value & 0xFF])


def _explicit(offset: int, method: int, index: int, disp: int | None = None) -> bytes:
    out = bytearray([0x80 | ((offset >> 8) & 3), offset & 0xFF])
    out.append((1 << 7) | (0 << 4) | (0x04 if disp is None else 0) | method)
    out += _as_index(index)
    if disp is not None:
        out += struct.pack("<H", disp)
    return bytes(out)


def _threaded(offset: int, number: int, disp: int | None = None) -> bytes:
    out = bytearray([0x80 | ((offset >> 8) & 3), offset & 0xFF])
    out.append((1 << 7) | (0 << 4) | 0x08 | (0x04 if disp is None else 0) | number)
    if disp is not None:
        out += struct.pack("<H", disp)
    return bytes(out)


def _walked(records: list[omf.Record]) -> list[tuple]:
    """Every fixup's decoded meaning, which a remap must leave alone but for
    the indices it was asked to change."""
    return [
        (one.seg, one.offset, one.loc, one.selfrel, one.target, one.index, one.disp)
        for one in omf.fixups(records)
    ]


def _with_fixups(*bodies: bytes) -> list[omf.Record]:
    ledata = omf.ledata_record(1, 0, bytes(64))
    return [ledata, omf.Record(omf.FIXUPP, b"".join(bodies))]


def test_renumbering_externals_leaves_an_identity_mapping_byte_identical() -> None:
    """Nothing to move, nothing rewritten: the encoding a record already has
    is the one it keeps, threads and all."""
    made = _with_fixups(_thread(False, 0, 2, 7), _threaded(0x10, 0), _explicit(0x20, 2, 9, 0))
    got = [omf.renumbered(one, {}) for one in made]
    assert [one.body for one in got] == [one.body for one in made]
    assert got[1] is made[1], "an untouched record is not copied"


def test_renumbering_moves_a_target_thread_and_the_fixups_that_use_it() -> None:
    """A thread names the external once and many fixups refer to it by
    number, so the index to move is in the thread and nowhere else."""
    made = _with_fixups(_thread(False, 0, 2, 9), _threaded(0x10, 0), _threaded(0x14, 0))
    before = _walked(made)
    got = [omf.renumbered(one, {9: 8}) for one in made]
    after = _walked(got)
    assert [one[5] for one in before] == [9, 9]
    assert [one[5] for one in after] == [8, 8]
    assert [one[:5] + one[6:] for one in before] == [one[:5] + one[6:] for one in after]


def test_renumbering_moves_externals_on_both_sides_of_a_removal() -> None:
    """One removed in the middle: everything after it steps down and
    everything before it stays where it was."""
    made = _with_fixups(_explicit(0x10, 2, 3, 0), _explicit(0x20, 2, 9, 0), _explicit(0x30, 2, 11, 0))
    got = [omf.renumbered(one, {9: 8, 11: 10}) for one in made]
    assert [one[5] for one in _walked(got)] == [3, 8, 10]


def test_renumbering_crosses_the_index_length_boundary() -> None:
    """An OMF index is one byte under 128 and two above it, so a remap
    across that line changes how many bytes the subrecord takes."""
    made = _with_fixups(_explicit(0x10, 2, 128, 0), _explicit(0x20, 2, 127, 0))
    got = [omf.renumbered(one, {128: 127, 127: 128}) for one in made]
    assert [one[5] for one in _walked(got)] == [127, 128]
    assert len(got[1].body) == len(made[1].body), "one grew and one shrank"


def test_renumbering_a_frame_that_names_an_external_moves_it_too() -> None:
    """A frame may be an external index just as a target may."""
    body = bytearray([0x80, 0x10])
    body.append((0 << 7) | (2 << 4) | 0x04 | 2)  # explicit frame method 2, target external
    body += _as_index(9) + _as_index(9)
    made = _with_fixups(bytes(body))
    got = [omf.renumbered(one, {9: 4}) for one in made]
    assert [one[5] for one in _walked(got)] == [4]
    assert omf.fixups(got)[0].frame == 4


def test_renumbering_the_corpus_moves_the_indices_and_nothing_else(obj: Path) -> None:
    """Real threaded data. Every external index shifted by one is a mapping
    that touches every naming there is, and nothing else about a fixup may
    move with it."""
    records = omf.parse(obj.read_bytes())
    every = {one.index for one in omf.fixups(records) if one.target == "external"}
    if not every:
        pytest.skip("no external fixups")
    mapping = {one: one + 1 for one in sorted(every, reverse=True)}
    moved = [omf.renumbered(one, mapping) for one in records]
    before, after = omf.fixups(records), omf.fixups(moved)
    assert len(before) == len(after)
    for one, other in zip(before, after, strict=True):
        assert (one.seg, one.offset, one.loc, one.selfrel, one.target, one.disp) == (
            other.seg, other.offset, other.loc, other.selfrel, other.target, other.disp
        )
        want = mapping.get(one.index, one.index) if one.target == "external" else one.index
        assert other.index == want


def test_a_thread_outlives_the_record_it_was_declared_in() -> None:
    """A thread is set in one FIXUPP record and used by fixups in the next.
    Remapping has to move the declaration and leave the references alone,
    or the second record's fixups resolve to the old external."""
    ledata = omf.ledata_record(1, 0, bytes(64))
    first = omf.Record(omf.FIXUPP, _thread(False, 0, 2, 9) + _threaded(0x10, 0))
    second = omf.Record(omf.FIXUPP, _threaded(0x20, 0) + _threaded(0x24, 0))
    made = [ledata, first, second]
    assert [one.index for one in omf.fixups(made)] == [9, 9, 9]
    got = [omf.renumbered(one, {9: 5}) for one in made]
    assert [one.index for one in omf.fixups(got)] == [5, 5, 5]
    assert got[2].body == second.body, "a record that only refers to a thread is untouched"


MARK = 0x9C  # a comment class no tool in this toolchain writes


def test_a_private_comment_survives_a_parse_and_emit() -> None:
    """The marker an emitted object carries, so a second run gives it back
    rather than raising generated code as if BC had written it.

    A COMENT with the purge and list bits set is one the linker discards
    from its output and never prints; an unknown class is data it does not
    read. BC's own are class 0x00, 0x9f and 0xa1, so 0x9c is free.
    """
    raw = (Path("fixtures/omf") / "hotlop-p-g2.obj").read_bytes()
    records = omf.parse(raw)
    marked = omf.finalised(records, "1:whole+absorb")
    assert omf.finalised_at(marked) == "1:whole+absorb"
    assert omf.finalised_at(records) is None
    out = b"".join(one.emit() for one in marked)
    assert omf.finalised_at(omf.parse(out)) == "1:whole+absorb"
    # And nothing else about the object moved.
    assert [one.type for one in omf.parse(out) if one.type & 0xFE != omf.COMENT] == [
        one.type for one in records if one.type & 0xFE != omf.COMENT
    ]


def test_a_second_marker_is_refused_rather_than_added(obj: Path) -> None:
    """One object, one account of what produced it. Two markers would say
    two different sets of options made the same bytes."""
    records = omf.finalised(omf.parse(obj.read_bytes()), "1:whole")
    with pytest.raises(ValueError, match="already"):
        omf.finalised(records, "1:whole")


def test_no_fixture_already_carries_the_marker_signature(obj: Path) -> None:
    """The class and signature have to be ones BC never writes, or an
    untouched object would be read back as one this pass already emitted."""
    assert omf.finalised_at(omf.parse(obj.read_bytes())) is None
