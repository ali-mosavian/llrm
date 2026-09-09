"""Floating arithmetic changes values even when stack depth does not change."""

from pathlib import Path

import corpus
import pytest

from qbopt import fpstack, ir, mir


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_fpcse_store_reads_product_not_original_load(tag):
    """FPCSE's (a+b)*c was reported as the original a because arithmetic minted no value."""
    path = Path(f"fixtures/omf/fpcse-{tag}.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    chain = next(block.ops for block in body.blocks if any(op.kind is mir.Kind.FMUL for op in block.ops))
    chain = [op for op in chain if op.stack is not None][:4]
    assert [op.kind for op in chain] == [mir.Kind.FLOAD, mir.Kind.FADD, mir.Kind.FMUL, mir.Kind.FSTORE]
    load, add, multiply, store = [fpstack.readings(body)[op.at] for op in chain]
    assert load.uses == {}
    assert add.defines is not None and multiply.defines is not None
    assert len({load.defines, add.defines, multiply.defines}) == 3
    assert add.uses[0] == load.defines
    assert multiply.uses[0] == add.defines
    assert store.uses[0] == multiply.defines
    assert store.popped == (multiply.defines,)


def test_arithmetic_pop_writes_destination_before_renumbering():
    """An arithmetic pop must leave its new result, not the previous destination, at the top."""
    def op(at, shape, kind, args, results, stack):
        return mir.Op(at, shape, kind.value, (), (), kind=kind, args=args, results=results, stack=stack)
    top, below = mir.Opaque(None, "st0"), mir.Opaque(None, "st1")
    ops = (
        op(0, ir.Operation.FLOAT_LOAD, mir.Kind.FLOAD, (mir.Const(0, 4),), (top,), 1),
        op(1, ir.Operation.FLOAT_LOAD, mir.Kind.FLOAD, (mir.Const(0, 4),), (top,), 1),
        op(2, ir.Operation.FLOAT_ARITH_POP, mir.Kind.FADD, (below, top), (below,), -1),
        op(3, ir.Operation.FLOAT_UNARY, mir.Kind.FNEG, (top,), (top,), 0),
    )
    readings = fpstack.readings(mir.MirBody(0, (mir.MirBlock(0, (), ops, ()),)))
    assert readings[2].uses == {1: readings[0].defines, 0: readings[1].defines}
    assert readings[2].defines is not None
    assert readings[2].popped == (readings[1].defines,)
    assert readings[3].uses == {0: readings[2].defines}
    assert readings[3].defines not in (None, readings[2].defines)


@pytest.mark.parametrize("boundary", ["call", "overflow"])
def test_unknown_stack_does_not_claim_value_reuse(boundary):
    """A call or a ninth outstanding push invalidates the floating value graph."""
    top = mir.Opaque(None, "st0")
    def push(at):
        return mir.Op(at, ir.Operation.FLOAT_LOAD, "fld", (), (), kind=mir.Kind.FLOAD,
                      args=(mir.Const(0, 4),), results=(top,), stack=1)
    ops = [push(0)]
    if boundary == "call":
        ops.append(mir.Op(1, ir.Operation.CALL, "call", (), (), kind=mir.Kind.CALL))
    else:
        ops.extend(push(at) for at in range(1, 9))
    ops.append(mir.Op(10, ir.Operation.FLOAT_UNARY, "fchs", (), (), kind=mir.Kind.FNEG,
                      args=(top,), results=(top,), stack=0))
    readings = fpstack.readings(mir.MirBody(0, (mir.MirBlock(0, (), tuple(ops), ()),)))
    assert readings[0].defines is not None
    assert readings[10].defines is None and readings[10].uses == {}


def test_stage_dump_exposes_floating_value_chain(capsys):
    """Pass dumps must distinguish the loaded value from each arithmetic result."""
    from tools import stages
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    stages._mir([("main", body)])
    output = capsys.readouterr().out
    assert "fp values: - -> f1@0x66" in output
    assert "fp values: f1@0x66 -> f2@0x6b" in output
    assert "fp values: f2@0x6b -> f3@0x70" in output
    assert "fp values: f3@0x70 -> -" in output
