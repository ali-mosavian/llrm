"""Whole values must exist before LICM, not be reconstructed after it."""

from dataclasses import replace
from pathlib import Path

import corpus
import pytest

from qbopt import ir, mir, raising_longs


@pytest.fixture
def nbody(monkeypatch):
    recognize = raising_longs.scalar
    with monkeypatch.context() as patch:
        patch.setattr(raising_longs, "scalar", lambda body: body)
        path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
        found = corpus.loaded(path)
        body = mir.bodies(found, corpus.partitioned(path))[0][1]
    return body, recognize


def test_position_arithmetic_is_scalar_before_optimization(nbody):
    """Nbody's four position halves blocked LICM; extracting them separately increased spills."""
    body, recognize = nbody
    done = recognize(body)
    ops = [op for block in done.blocks for op in block.ops]
    load = next(op for op in ops if op.at == 0x11d)
    subtract = next(op for op in ops if op.at == 0x12d)
    store = next(op for op in ops if op.at == 0x135)
    assert load.kind is mir.Kind.LOAD and load.results[0].width == 4
    assert subtract.kind is mir.Kind.SUB and subtract.args[0] == load.results[0]
    assert subtract.loads[0].width == 4
    assert store.kind is mir.Kind.STORE and store.args == subtract.results
    assert store.stores[0].width == 4


@pytest.mark.parametrize("producer", [0x12d, 0x131])
def test_live_half_flags_prevent_scalar_arithmetic(nbody, producer):
    body, recognize = nbody
    block = next(block for block in body.blocks if block.at == 0x117)
    original = next(op for op in block.ops if op.at == producer)
    flag = next(value for value in original.defines if value.flags)
    reader = mir.Op(0x217, ir.Operation.JUMP, "jz", (), (flag,), kind=mir.Kind.BRANCH)
    changed = replace(block, ops=(*block.ops, reader))
    body = replace(body, blocks=tuple(changed if one is block else one for one in body.blocks))
    done = recognize(body)
    subtract = next(op for block in done.blocks for op in block.ops if op.at == 0x12d)
    assert subtract.results[0].width == 2


def test_same_machine_address_with_different_ssa_base_is_not_a_pair(nbody):
    body, recognize = nbody
    def changed(op):
        if op.at != 0x121:
            return op
        ref = replace(op.loads[0], base=mir.Value(99999, op.at))
        return replace(op, loads=(ref,), args=(mir.Cell(ref),))
    body = replace(body, blocks=tuple(replace(block, ops=tuple(map(changed, block.ops))) for block in body.blocks))
    done = recognize(body)
    load = next(op for block in done.blocks for op in block.ops if op.at == 0x11d)
    assert load.results[0].width == 2
