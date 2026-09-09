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


def test_proven_allocation_supplies_segment_identity() -> None:
    """A proved array pointer still reloaded the value just stored through the same offset."""
    allocation = mir.Symbol(Space.SEGMENT, 5, 6, 2)
    ref = mir.MemRef(Addr(Space.FAR, 0), 2, mir.Value(1, 0), allocation=allocation)
    assert mir.same_bytes(ref, ref)
    assert not mir.same_bytes(ref, replace(ref, allocation=None))
    assert not mir.same_bytes(ref, replace(ref, allocation=replace(allocation, offset=32)))


def test_harr_uses_the_value_it_just_stored() -> None:
    """HARR loaded each element immediately after storing its already-available value."""
    from pathlib import Path
    import corpus
    from qbopt import transform

    path = Path("fixtures/omf/harr-p-g2.obj")
    module = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(module, partition)[0][1]
    result = transform.applied(body, module.dgroup, module.calls, blocks=partition, found=module)
    assert any(ref.allocation for block in result.blocks for op in block.ops for ref in op.stores)
    assert not any(ref.allocation for block in result.blocks for op in block.ops for ref in op.loads)
