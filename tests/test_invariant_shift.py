"""FPBENCH recomputed body * 4 on every inner interaction instead of once per body."""

from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt.abi import runtime
from qbopt.frontend import blocks
from qbopt.model import mir
from qbopt.optimize import transform


def test_fpbench_outer_index_is_not_recomputed_in_inner_loop():
    found = corpus.loaded(Path("fixtures/bench/fpbench-v-g3.obj"))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition, runtime.for_module(found))[0][1]
    original = next(op for block in body.blocks for op in block.ops if op.at == 0x108)
    assert original.kind is mir.Kind.SHL
    optimized = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    inner = next(loop for loop in transform.loopy.loops(list(optimized.blocks), optimized.entry)
                 if loop.header == 0x18c)
    remaining = [op for block in optimized.blocks if block.at in inner.body
                 for op in block.ops if op.kind is mir.Kind.SHL and op.id == original.id]
    assert not remaining
    assert any(op.kind is mir.Kind.SHL and op.id == original.id
               for block in optimized.blocks if block.at not in inner.body for op in block.ops)
    assert any(op.kind is mir.Kind.SHL for block in optimized.blocks if block.at in inner.body
               for op in block.ops)


@pytest.mark.parametrize("guard", ["live_flags", "partial", "merge", "zero", "wide", "unknown"])
def test_shift_exception_requires_a_complete_value_and_unused_flags(guard):
    from qbopt.model import ir
    source, result, flags = mir.Value(1, 0), mir.Value(2, 1), mir.Value(3, 1, flags=True)
    op = mir.Op(1, ir.Operation.BINARY, "shl", (result, flags), (source,),
                kind=mir.Kind.SHL, args=(mir.Held(source, 2), mir.Const(2, 1)),
                results=(mir.Held(result, 2),))
    readable = {result}
    assert transform._whole_shift(op, readable)
    match guard:
        case "live_flags":
            readable.add(flags)
        case "partial":
            op = replace(op, results=(mir.Held(result, 1),))
        case "merge":
            op = replace(op, merges={result: source})
        case "zero" | "wide":
            op = replace(op, args=(op.args[0], mir.Const(0 if guard == "zero" else 16, 1)))
        case "unknown":
            readable = None
    assert not transform._whole_shift(op, readable)
