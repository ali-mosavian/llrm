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
