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
