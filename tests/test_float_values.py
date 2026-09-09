"""Floating MIR uses value edges, not rotating stack-slot names."""

from pathlib import Path
from dataclasses import replace

import corpus
import pytest

from qbopt.model import mir
from qbopt.backend import lower_floats
from qbopt.frontend import raising_float_values
from qbopt import wholeseg


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_fpdeep_integer_results_are_explicit_values(tag):
    """FPDEEP's CLNG results were opaque calls, disconnecting FP values from PRINT arguments."""
    from qbopt.model.floating import Format
    path = Path(f"fixtures/omf/fpdeep-{tag}.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    conversions = [op for block in body.blocks for op in block.ops
                   if op.floating is not None and op.floating.result is Format.SIGNED32]
    assert len(conversions) == 5
    assert all(isinstance(op.args[0], mir.Held) and op.args[0].width == 10 for op in conversions)
    assert all(isinstance(op.results[0], mir.Held) and op.results[0].width == 4 for op in conversions)
    emitted = wholeseg.emitted(path.read_bytes())
    assert emitted.outcome is wholeseg.Emission.LIR, emitted.reason
    from qbopt.objectfile import module, omf
    assert "B$FIST" not in module.of(omf.parse(emitted.data)).calls.values()


@pytest.mark.parametrize("guard", ["contract", "writes", "error", "flags"])
def test_float_integer_recognition_requires_the_full_helper_contract(guard, monkeypatch):
    from qbopt.abi import runtime
    from qbopt.frontend import raising_float_results
    path = Path("fixtures/omf/fpdeep-p-g2.obj")
    found = corpus.loaded(path)
    rules = runtime.for_module(found)
    recognize = raising_float_results.raised
    monkeypatch.setattr(raising_float_results, "raised", lambda body, *args: body)
    body = mir.bodies(found, corpus.partitioned(path), rules)[0][1]
    call = next(op for block in body.blocks for op in block.ops if found.calls.get(op.at) == "B$FIST")
    if guard == "flags":
        flag = next(value for value in call.defines if value.flags)
        body = replace(body, blocks=tuple(replace(block, ops=tuple(
            replace(op, uses=(*op.uses, flag)) if op.at == call.covers[1] else op for op in block.ops
        )) for block in body.blocks))
    else:
        changes = {"contract": {"established": False}, "writes": {"writes": runtime.Memory.ANY},
                   "error": {"raises_error": True}}
        rules = {**rules, call.at: replace(rules[call.at], **changes[guard])}
    result = recognize(body, found, rules)
    assert next(op for block in result.blocks for op in block.ops if op.id == call.id).kind is mir.Kind.CALL


def _allocated(body, path):
    from qbopt.backend import floatalloc, lower
    from qbopt.abi import runtime
    found = corpus.loaded(path)
    return floatalloc.allocated(lower.lowered("test", body, found.calls, found.absorbed, runtime.for_module(found)))


def test_lowering_can_preserve_a_shared_sum_after_redundant_float_ops_are_removed():
    """FPCSE recomputed a+b because lowering required every original FP instruction."""
    from qbopt.analysis import ssa
    from qbopt.model import ir

    path = Path("fixtures/omf/fpcse-p-g2.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    block = next(block for block in body.blocks if any(op.floating_origin for op in block.ops))
    floats = [op for op in block.ops if op.floating_origin]
    first_sum = floats[1].results[0].value
    second_sum = floats[5].results[0].value
    removed = {floats[4].id, floats[5].id}
    ops = tuple(replace(op, op=ir.Operation.NOTHING, kind=mir.Kind.NOTHING,
                        name="", args=(), results=(), uses=(), defines=(), loads=(),
                        stores=(), merges={}, node=None, made=None, raised=None,
                        floating=None, stack=None)
                if op.id in removed else ssa.substituted(op, {second_sum.id: first_sum})
                for op in block.ops)
    changed = replace(body, blocks=tuple(replace(one, ops=ops) if one is block else one for one in body.blocks))
    low = _allocated(changed, path)
    names = [one.what.name for one in low.insns if one.what]
    assert names.count("fadd") == 3  # shared sum plus the two accumulator additions
    assert any(one.what and one.what.name == "fld" and one.what.sources == (ir.St(0),)
               for one in low.insns), "the shared sum must survive the destructive multiply"


def test_floating_values_survive_lowering_until_allocation():
    """FPCSE lowering must not assign physical stack registers ahead of allocation."""
    from qbopt.model import ir
    from qbopt.backend import lower
    from qbopt.abi import runtime
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    low = lower.lowered("test", body, found.calls, found.absorbed, runtime.for_module(found))
    operations = [one for block in low.blocks for one in block.insns if one.op.floating_origin]
    assert operations
    assert all(any(isinstance(arg, ir.Held) and arg.width == 10
                   for arg in (*one.what.sources, *one.what.dests)) for one in operations)
    allocated = _allocated(body, path)
    assert not any(isinstance(arg, ir.Held) and arg.width == 10 for block in allocated.blocks
                   for one in block.insns if one.what for arg in (*one.what.sources, *one.what.dests))


def test_removed_float_operation_cannot_retain_hidden_computation():
    """A deletion marker must not replay FPCSE's original load through its node."""
    from qbopt.backend.lower import Unlowered
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    op = next(op for block in body.blocks for op in block.ops if op.floating_origin)
    with pytest.raises(Unlowered, match="retains computation"):
        lower_floats.operation(replace(op, kind=mir.Kind.NOTHING))


def test_cse_reuses_exact_fpcse_sum():
    """FPCSE performed a+b twice despite unchanged operands and exact FP work."""
    from qbopt.optimize import transform
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    changed = transform.subexpressions(body, found.dgroup)
    assert sum(op.kind is mir.Kind.FADD for block in changed.blocks for op in block.ops) == 3
    _allocated(changed, path)
    emitted = wholeseg.emitted(path.read_bytes())
    assert emitted.outcome is wholeseg.Emission.LIR, emitted.reason
    from qbopt.frontend import blocks
    from qbopt.objectfile import module, omf
    instructions = blocks.instructions(module.of(omf.parse(emitted.data)))
    assert not isinstance(instructions, str)
    assert sum(str(one.insn).split()[0] in {"fadd", "faddp"} for one in instructions) == 3


@pytest.mark.parametrize("guard", [None, "unknown", "rounding", "barrier", "alias"])
def test_exact_store_can_supply_a_later_floating_load(guard, monkeypatch):
    """Controlled FPCSE witness: an exact stored product was loaded again instead of kept alive."""
    from qbopt.analysis import floatfacts
    from qbopt.optimize import transform
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    block = next(block for block in body.blocks if any(op.floating for op in block.ops))
    floats = [op for op in block.ops if op.floating]
    store, load = floats[3:5]
    reload = replace(load, args=store.results, loads=store.stores)
    ops = list(block.ops)
    ops[ops.index(load)] = reload
    if guard in ("barrier", "alias"):
        marker = replace(store, floating=None, stack=None,
                         kind=mir.Kind.OPAQUE if guard == "barrier" else mir.Kind.STORE,
                         stores=store.stores if guard == "alias" else ())
        ops.insert(ops.index(reload), marker)
    body = replace(body, blocks=tuple(replace(one, ops=tuple(ops)) if one is block else one
                                     for one in body.blocks))
    if guard in ("unknown", "rounding"):
        facts = floatfacts.known(body, found.dgroup, {})
        if guard == "unknown":
            facts.pop(store.args[0].value)
        else:
            from fractions import Fraction
            facts[store.args[0].value] = floatfacts.Finite(Fraction(1, 3))
        monkeypatch.setattr(floatfacts, "known", lambda *args: facts)
    changed = transform.subexpressions(body, found.dgroup)
    survivor = next(op for one in changed.blocks for op in one.ops if op.id == load.id)
    assert (survivor.kind is mir.Kind.NOTHING) == (guard is None)
    if guard is None:
        before = _allocated(body, path)
        after = _allocated(changed, path)
        assert any(one.at == load.at and one.what and one.what.name == "fld" for one in before.insns)
        assert not any(one.at == load.at and one.what and one.what.name == "fld" for one in after.insns)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_stored_fpcse_products_feed_additions_without_memory_reads(tag):
    """FPCSE reloaded its exact stored products as memory operands of its final additions."""
    from qbopt.optimize import transform
    path = Path(f"fixtures/omf/fpcse-{tag}.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    changed = transform.forwarded(body, found.dgroup, found.calls)
    adds = [op for block in changed.blocks for op in block.ops if op.kind is mir.Kind.FADD]
    assert all(op.loads for op in adds)  # Unknown loop accumulator is not a proof.
    result = wholeseg.emitted(path.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert sum(one.split()[0] == "faddp" for one in instructions) == 2
    assert sum(one.split()[0] == "fxch" for one in instructions) == 1


@pytest.mark.parametrize("change", ["unknown_effect", "barrier", "alias", "rounding"])
def test_float_cse_preserves_computations_without_reuse_proof(change, monkeypatch):
    """FPCSE's shared sum is not reusable across unknown effects or changed memory."""
    from qbopt.analysis import floatfacts
    from qbopt.model.floating import Rounding
    from qbopt.optimize import transform
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    block = next(block for block in body.blocks if any(op.floating_origin for op in block.ops))
    floats = [op for op in block.ops if op.floating_origin]
    if change == "unknown_effect":
        facts = floatfacts.known(body, found.dgroup, {})
        facts.pop(floats[2].results[0].value)
        monkeypatch.setattr(floatfacts, "known", lambda *args, **kwargs: facts)
        from qbopt.analysis import floatbounds
        bounded = floatbounds.exact
        monkeypatch.setattr(floatbounds, "exact", lambda *args: bounded(*args) - {id(floats[2])})
    else:
        target = floats[5] if change == "rounding" else floats[3]
        match change:
            case "barrier":
                altered = replace(target, kind=mir.Kind.OPAQUE)
            case "alias":
                altered = replace(target, stores=floats[0].loads)
            case "rounding":
                altered = replace(target, floating=replace(target.floating, rounding=Rounding.NONE))
        body = replace(body, blocks=tuple(replace(one, ops=tuple(altered if op is target else op for op in one.ops))
                                         if one is block else one for one in body.blocks))
    changed = transform.subexpressions(body, found.dgroup)
    assert sum(op.kind is mir.Kind.FADD for block in changed.blocks for op in block.ops) == 4


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
    from qbopt.backend.lower import Unlowered
    from qbopt.model.floating import Rounding
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
        _allocated(changed, path)


def test_generic_memory_reuse_respects_float_conversion_and_effects():
    """An extended producer is not the rounded SINGLE stored by FPCSE."""
    from qbopt.analysis import avail
    from qbopt.optimize import transform
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
    from qbopt.backend import lower
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    op = next(op for block in body.blocks for op in block.ops if op.kind is mir.Kind.FMUL)
    assert lower.current(op) == op.node.semantics
    with pytest.raises(lower.Unlowered, match="floating dataflow"):
        lower.current(replace(op, args=(replace(op.args[0], value=mir.Value(9999, 0, variable=9999)), *op.args[1:])))


@pytest.mark.parametrize("change", ["missing_push", "premature_pop", "wrong_slot"])
def test_float_lowering_checks_actual_stack_transitions(change):
    """FPCSE must not consume an empty or different slot despite unchanged SSA names."""
    from qbopt.backend.lower import Unlowered
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
    baseline = _allocated(body, path)
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
    allocated = _allocated(changed, path)
    assert [(one.what, one.uses, one.defines) for block in allocated.blocks for one in block.insns] == [
        (one.what, one.uses, one.defines) for block in baseline.blocks for one in block.insns]
