"""FPDEEP's three printed iterations cannot be replaced by one final iteration."""

from pathlib import Path

import pytest
import corpus
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.analysis import floatfacts
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


def test_unrolled_latch_explicitly_skips_the_original_header():
    """FPDEEP fell through to its original header and started the expanded body again."""
    found, original = body()
    loop, = loops.loops(original.blocks, original.entry)
    changed = unroll.expanded(original, found.dgroup, found.calls)
    latch = changed.block(next(iter(loop.latches)))
    assert latch.ops[-1].kind is mir.Kind.JUMP
    assert latch.ops[-1].target == latch.succ[0]
    assert latch.ops[-1].target not in loop.body


def test_fpdeep_expansion_exposes_exact_array_arithmetic():
    """FPDEEP's 144/784/3600 squares and 6/14/30 ratios stayed unknown after expansion."""
    found, original = body()
    changed = unroll.expanded(original, found.dgroup, found.calls)
    facts = floatfacts.known(changed, found.dgroup, found.calls)
    for address, expected in ((0x9c, [144, 784, 3600]), (0xe4, [6, 14, 30])):
        values = [facts.get(op.args[0].value) for block in changed.blocks for op in block.ops
                  if op.at == address and op.kind is mir.Kind.FSTORE and not op.stores]
        assert all(value is not None for value in values)
        assert [value.value for value in values] == expected


def test_emission_must_not_accept_unrolled_provenance_yet():
    """FPDEEP timed out when repeated input addresses interleaved its calls and lost fixups."""
    found, original = body()
    changed = unroll.expanded(original, found.dgroup, found.calls)
    with pytest.raises(lower.Unlowered, match="floating sequence changed"):
        lower_floats.checked(changed)
