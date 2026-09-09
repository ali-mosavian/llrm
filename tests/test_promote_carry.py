"""Carry arithmetic can reuse a stored value without changing its condition input."""

from pathlib import Path

import corpus
from qbopt import mir
from qbopt import transform


def test_lngmix_high_accumulator_is_a_value() -> None:
    """LNGMIX reloaded the high accumulator word on every one of ten iterations."""
    path = Path("fixtures/omf/lngmix-p-g2.obj")
    found = corpus.loaded(path)
    blocks = corpus.partitioned(path)
    body = mir.bodies(found, blocks)[0][1]
    body = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
    operations = [op for block in body.blocks for op in block.ops if op.kind is mir.Kind.ADD_CARRY]
    assert operations
    assert all(not op.loads and not any(isinstance(arg, mir.Cell) for arg in op.args) for op in operations)
    assert all(any(value.flags for value in op.uses) for op in operations)
