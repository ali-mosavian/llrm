"""An unchanged offset is not an unchanged far pointer."""

from dataclasses import replace

import pytest

from qbopt import avail, mir
from qbopt.module import Addr, Space


@pytest.mark.parametrize("segment", [None, mir.Value(3, 0)])
def test_unknown_or_changed_segment_cannot_forward_or_cover(segment: mir.Value | None) -> None:
    """Far stores could cover another segment's store merely because the offset matched."""
    old = mir.MemRef(Addr(Space.FAR, 0), 2, mir.Value(1, 0), segment)
    new = replace(old, segment=None if segment is None else mir.Value(3, 1))
    assert not mir.same_bytes(old, new)
    assert not avail._covered_by(old, new, frozenset())


def test_known_segment_and_offset_can_forward_and_cover() -> None:
    ref = mir.MemRef(Addr(Space.FAR, 0), 2, mir.Value(1, 0), mir.Value(3, 0))
    assert mir.same_bytes(ref, ref)
    assert avail._covered_by(ref, replace(ref, width=4), frozenset())
    assert not avail._covered_by(replace(ref, width=4), ref, frozenset())


def test_different_relocated_objects_do_not_cover_each_other() -> None:
    """Equal offsets in two object segments incorrectly made the earlier store dead."""
    old = mir.MemRef(Addr(Space.SEGMENT, 6, 5), 2)
    new = mir.MemRef(Addr(Space.SEGMENT, 6, 6), 2)
    assert not avail._covered_by(old, new, frozenset({5, 6}))
