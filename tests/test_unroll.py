"""FPDEEP's three printed iterations cannot be replaced by one final iteration."""

from pathlib import Path

import pytest
import corpus
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.backend import lower, lower_floats
from qbopt.optimize import transform, unroll


def body():
    path = Path("fixtures/omf/fpdeep-p-g2.obj")
    found = corpus.loaded(path)
    original = mir.bodies(found, corpus.partitioned(path))[0][1]
    optimized = transform.applied(original, found.dgroup, found.calls, found=found)
    return found, optimized


def test_fpdeep_unroll_preserves_order_and_fresh_definitions():
    found, original = body()
    loop, = loops.loops(original.blocks, original.entry)
    latch = original.block(next(iter(loop.latches)))
    changed = unroll.expanded(original, found.dgroup, found.calls)
    assert not loops.loops(changed.blocks, changed.entry)
    operations = changed.block(latch.at).ops
    effects = lambda ops: [op.id for op in ops if op.floating or op.kind is mir.Kind.CALL]
    assert effects(operations) == effects(latch.ops) * 3
    definitions = [value.id for op in operations for value in op.defines]
    assert len(definitions) == len(set(definitions))
    assert unroll.expanded(changed, found.dgroup, found.calls) == changed


def test_emission_must_not_accept_unrolled_provenance_yet():
    """FPDEEP timed out when repeated input addresses interleaved its calls and lost fixups."""
    found, original = body()
    changed = unroll.expanded(original, found.dgroup, found.calls)
    with pytest.raises(lower.Unlowered, match="floating sequence changed"):
        lower_floats.checked(changed)
