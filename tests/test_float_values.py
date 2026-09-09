"""Floating MIR uses value edges, not rotating stack-slot names."""

from pathlib import Path
from dataclasses import replace

import corpus
import pytest

from qbopt import mir, lower_floats, raising_float_values, wholeseg


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_fpcse_float_values_link_each_computation(tag):
    """FPCSE's opaque st0 operands concealed all cross-operation data dependencies."""
    path = Path(f"fixtures/omf/fpcse-{tag}.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    operations = [op for block in body.blocks for op in block.ops if op.floating is not None]
    assert operations and all(op.floating_origin is not None for op in operations)
    assert not any(isinstance(arg, mir.Opaque) for op in operations for arg in (*op.args, *op.results))
    start = next(index for index, op in enumerate(operations) if op.kind is mir.Kind.FADD) - 1
    load, add, multiply, store = operations[start:start+4]
    assert add.args[0] == load.results[0]
    assert multiply.args[0] == add.results[0]
    assert store.args[0] == multiply.results[0]
    assert all(op.results[0].value in op.defines for op in (load, add, multiply))
    variables = {op.results[0].value.variable for op in operations if op.results and isinstance(op.results[0], mir.Held)}
    assert not variables.intersection(value.variable for block in body.blocks for op in block.ops
                                      if op.floating_origin is None for value in (*op.uses, *op.defines))
    result = wholeseg.emitted(path.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason


@pytest.mark.parametrize("change", ["operand", "order", "rounding"])
def test_unimplemented_float_rewrites_cannot_silently_use_old_code(change):
    from qbopt.lower import Unlowered
    from qbopt.floating import Rounding
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    block = next(block for block in body.blocks if any(op.floating_origin for op in block.ops))
    ops = list(block.ops)
    indices = [index for index, op in enumerate(ops) if op.floating_origin]
    first, second = indices[:2]
    if change == "order":
        ops[first], ops[second] = ops[second], ops[first]
    elif change == "rounding":
        ops[second] = replace(ops[second], floating=replace(ops[second].floating, rounding=Rounding.NONE))
    else:
        arg = ops[second].args[0]
        ops[second] = replace(ops[second], args=(replace(arg, value=mir.Value(9999, 0, variable=9999)), *ops[second].args[1:]))
    changed = replace(body, blocks=tuple(replace(one, ops=tuple(ops)) if one is block else one for one in body.blocks))
    with pytest.raises(Unlowered, match="floating"):
        lower_floats.restored(changed)


def test_generic_memory_reuse_respects_float_conversion_and_effects():
    """An extended producer is not the rounded SINGLE stored by FPCSE."""
    from qbopt import avail, transform
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    operations = [op for block in body.blocks for op in block.ops if op.floating_origin]
    load = next(op for op in operations if op.kind is mir.Kind.FLOAD)
    store = next(op for op in operations if op.kind is mir.Kind.FSTORE)
    assert avail.loaded_into(load) is None
    assert avail.stored_from(store) is None
    assert avail.stored_cell(store) is None
    assert transform._served(load, store.args[0].value) is None


def test_direct_lowering_uses_the_same_float_baseline():
    """Legacy instruction consumers must not mistake a floating value for a general register."""
    from qbopt import lower
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    op = next(op for block in body.blocks for op in block.ops if op.kind is mir.Kind.FMUL)
    assert lower.current(op) == op.node.semantics
    with pytest.raises(lower.Unlowered, match="floating dataflow"):
        lower.current(replace(op, args=(replace(op.args[0], value=mir.Value(9999, 0, variable=9999)), *op.args[1:])))


@pytest.mark.parametrize("change", ["missing_push", "premature_pop", "wrong_slot"])
def test_float_lowering_checks_actual_stack_transitions(change):
    """FPCSE must not consume an empty or different slot despite unchanged SSA names."""
    from qbopt.lower import Unlowered
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    block = next(block for block in body.blocks if any(op.floating_origin for op in block.ops))
    ops = list(block.ops)
    index = next(index for index, op in enumerate(ops)
                 if op.kind is (mir.Kind.FLOAD if change == "missing_push" else mir.Kind.FADD))
    op = ops[index]
    if change == "wrong_slot":
        origin = op.floating_origin
        ops[index] = replace(op, floating_origin=replace(origin,
            machine_inputs=(mir.Opaque(None, "st1"), *origin.machine_inputs[1:])))
    else:
        ops[index] = replace(op, stack=0 if change == "missing_push" else -1)
    changed = replace(body, blocks=tuple(replace(one, ops=tuple(ops)) if one is block else one for one in body.blocks))
    with pytest.raises(Unlowered, match="floating stack"):
        lower_floats._stack_checked(next(one for one in changed.blocks if one.at == block.at))


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_float_allocation_uses_current_ssa_values(tag):
    """FPCSE allocation must follow renamed value edges, not BC's origin identifiers."""
    path = Path(f"fixtures/omf/fpcse-{tag}.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    baseline = lower_floats.restored(body)
    variables = {arg.value.variable for block in body.blocks for op in block.ops
                 if op.floating_origin for arg in (*op.args, *op.results)
                 if isinstance(arg, mir.Held) and arg.width == 10}
    def renamed(value):
        return replace(value, id=value.id + 10000, variable=value.variable + 10000) if value.variable in variables else value
    def operand(arg):
        return replace(arg, value=renamed(arg.value)) if isinstance(arg, mir.Held) else arg
    changed = replace(body, blocks=tuple(replace(block, ops=tuple(replace(op,
        args=tuple(map(operand, op.args)), results=tuple(map(operand, op.results)),
        uses=tuple(map(renamed, op.uses)), defines=tuple(map(renamed, op.defines)))
        for op in block.ops)) for block in body.blocks))
    allocated = lower_floats.restored(changed)
    assert [(op.args, op.results, op.uses, op.defines) for block in allocated.blocks for op in block.ops] == [
        (op.args, op.results, op.uses, op.defines) for block in baseline.blocks for op in block.ops]
