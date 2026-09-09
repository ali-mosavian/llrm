from dataclasses import replace

from iced_x86 import Register

from qbopt.model import mir
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def test_equal_offsets_do_not_prove_far_segments_disjoint() -> None:
    """1000:0020 and 1001:0010 name the same byte despite different offsets."""
    base = mir.Value(1, 0)
    one = mir.MemRef(Addr(Space.FAR, 0x20, base=Register.BX), 2, base, mir.Value(2, 0))
    other = mir.MemRef(Addr(Space.FAR, 0x10, base=Register.BX), 2, base, mir.Value(3, 0))
    assert mir.overlapping(one, other, frozenset())
    assert mir.overlapping(other, one, frozenset())
    assert not mir.overlapping(one, replace(other, segment=one.segment), frozenset())


def test_unknown_far_segments_cannot_use_offset_disjointness() -> None:
    base = mir.Value(1, 0)
    one = mir.MemRef(Addr(Space.FAR, 0x20, base=Register.BX), 2, base)
    other = replace(one, addr=one.addr.plus(16))
    assert mir.overlapping(one, other, frozenset())


def test_equal_index_values_do_not_establish_equal_segment_origins() -> None:
    base = mir.Value(1, 0)
    one = mir.MemRef(Addr(Space.SEGMENT, 0x20, 1, Register.SI), 2, base)
    other = mir.MemRef(Addr(Space.SEGMENT, 0x10, 2, Register.SI), 2, base)
    assert mir.overlapping(one, other, frozenset({1, 2}))
