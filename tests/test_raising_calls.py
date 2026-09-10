from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt.backend import asm
from qbopt.model import ir, mir
from qbopt.objectfile import module, omf
from qbopt.frontend import pairs, raising_calls
from qbopt.optimize import transform
from qbopt import wholeseg
from qbopt.legacy import calls
from qbopt.abi import runtime


def test_nbody_timer_does_not_keep_arithmetic_scratch_values_live():
    """NBODY kept Y damping's DVI4 because PITSNAP invented register arguments."""
    path = Path("fixtures/bench/nbody-v-g3.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    timers = [op for op in ops if op.kind is mir.Kind.CALL and found.calls.get(op.at) == "PITSNAP"]
    assert len(timers) == 2
    assert all(not op.uses and op.defines for op in timers)
    assert any(op.at == 0x26e and op.kind is mir.Kind.DIVMOD for op in ops)
    emitted = wholeseg.emitted(path.read_bytes())
    assert emitted.outcome is wholeseg.Emission.LIR, emitted.reason
    rewritten = module.of(omf.parse(emitted.data))
    assert calls.DIVIDE not in rewritten.calls.values()
    assert list(rewritten.calls.values()).count(calls.MULTIPLY) == 1  # Timer conversion only.


@pytest.mark.parametrize("inputs", [None, frozenset(), frozenset({runtime.Reg.CX})])
def test_known_call_inputs_do_not_establish_unknown_call_effects(inputs):
    """Fixing NBODY's phantom timer inputs must not invent preserved registers."""
    contract = replace(runtime.worst("unresolved"), inputs=inputs)
    touched = mir._call_touches("unresolved", contract)
    if inputs is None:
        assert touched is None
    else:
        defines, uses = touched
        assert defines == frozenset(mir.TRACKED) | {mir.FLAGS}
        assert uses == frozenset(mir.FROM_CONTRACT[one] for one in inputs)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("name", ["lngmix", "lngmxx"])
def test_long_division_setup_is_not_counted_as_stack_arguments(tag, name):
    """QB LNGMIX kept two runtime divisions per iteration because MOV/CWD setup polluted push grouping."""
    path = Path(f"fixtures/omf/{name}-{tag}.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(found, partition)[0][1]
    assert not any(op.kind is mir.Kind.CALL and found.calls.get(op.at) in calls.DIVIDES
                   for block in body.blocks for op in block.ops)
    result = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    divisions = [op for block in result.blocks for op in block.ops if op.kind is mir.Kind.DIVMOD]
    assert len(divisions) == (0 if name == "lngmix" else 1)
    emitted = wholeseg.emitted(path.read_bytes())
    assert emitted.outcome is wholeseg.Emission.LIR, emitted.reason
    rewritten = module.of(omf.parse(emitted.data))
    assert not calls.DIVIDES.intersection(rewritten.calls.values())


def test_nbody_all_runtime_multiplies_are_scalar_values():
    """Nbody's classified memory multiplies remained frozen machine sites, blocking forwarding."""
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    for at, name in found.calls.items():
        if name != calls.MULTIPLY:
            continue
        op = next(op for block in body.blocks for op in block.ops if op.at == at and op.kind is mir.Kind.MUL)
        assert op.node is None and not op.loads
        assert len(op.results) == 1 and op.results[0].width == 4
        assert all(isinstance(arg, mir.Held) and arg.width == 4 for arg in op.args)


@pytest.mark.parametrize("separated", [False, True])
def test_memory_argument_capture_requires_adjacent_pushes(separated):
    """A separated pair must keep its original snapshots, not reread both words at the second push."""
    low = mir.MemRef(module.Addr(module.Space.SEGMENT, 4, 5), 2)
    high = replace(low, addr=low.addr.plus(2))
    def push(at, ref):
        return mir.Op(at, ir.Operation.PUSH, "push", (), (), kind=mir.Kind.ARG,
                      args=(mir.Cell(ref),), loads=(ref,), covers=(at, at+3))
    answer = raising_calls._whole_memory([push(0, high), push(6 if separated else 3, low)])
    assert answer == (None if separated else replace(low, width=4))


def test_unused_loop_clobbers_do_not_hide_nbody_division() -> None:
    """Nbody's final force division stayed opaque solely because dead phis carried call clobbers."""
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    assert any(op.at == 0x204 and op.kind is mir.Kind.DIVMOD for op in ops)
    assert not any(op.at == 0x204 and op.kind is mir.Kind.CALL for op in ops)


def test_widening_does_not_move_nbody_store_before_its_definition() -> None:
    """PDS nbody printed PX0=6137536 for 1258 after widening moved DELTAY before its subtract."""
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    blocks = corpus.partitioned(path)
    body = mir.bodies(found, blocks)[0][1]
    body = pairs.widened(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
    ops = next(block.ops for block in body.blocks if block.at == 0x117)
    store = next(op for op in ops if op.at == 0x150 and op.kind is mir.Kind.STORE)
    source = store.args[0].value
    assert next(index for index, op in enumerate(ops) if source in op.defines) < ops.index(store)
    difference = next(op for op in ops if op.at == 0x12d and op.kind is mir.Kind.SUB)
    multiply = next(op for op in ops if op.at == 0x1cd and op.kind is mir.Kind.MUL)
    assert difference.results[0].width == 4
    assert difference.results[0] in multiply.args
    assert ops.index(difference) < ops.index(multiply)


def test_divide_relocation_survives_index_value_replacement() -> None:
    """Nbody refused its velocity divide after LICM renamed an index without changing its relocation."""
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    # The scalar divide now owns no address; its argument capture owns it.
    # Keep exercising the legacy operand-binding guard on that real operand.
    op = next(op for block in body.blocks for op in block.ops if op.at == 0x267 and op.kind is mir.Kind.LOAD)
    op = replace(op, raised=(op.args, op.results))
    fields = frozenset(one.offset for one in omf.fixups(found.records) if one.seg == found.seg)
    expected = asm._divide_fields(op, found, fields)
    assert expected
    cell = op.args[0]
    moved = replace(cell, ref=replace(cell.ref, base=mir.Value(99999, 0)))
    renamed = replace(op, args=(moved, *op.args[1:]))
    assert asm._divide_fields(renamed, found, fields) == expected
    different = replace(moved, ref=replace(moved.ref, addr=moved.ref.addr.plus(4)))
    assert asm._divide_fields(replace(op, args=(different, *op.args[1:])), found, fields) is None


def test_nbody_classified_divide_consumes_captured_values() -> None:
    """Forwarded nbody velocity refused at 0x25f when frozen division required memory."""
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    divide = next(op for op in ops if op.at == 0x26b and op.kind is mir.Kind.DIVMOD)
    assert divide.node is None and not divide.loads
    assert all(isinstance(arg, mir.Held) and arg.width == 4 for arg in divide.args)
    captured = next(op for op in ops if op.at == 0x267 and op.kind is mir.Kind.LOAD)
    assert captured.results[0] == divide.args[0]


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_computed_divisions_are_values_not_runtime_calls(tag: str) -> None:
    path = Path(f"fixtures/omf/chain-{tag}.obj")
    found = corpus.loaded(path)
    bodies = mir.bodies(found, corpus.partitioned(path))
    divisions = [
        op
        for _, body in bodies
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.DIVMOD and op.node is None
    ]
    assert divisions
    for op in divisions:
        assert len(op.args) == len(op.results) == 2
        assert all(isinstance(arg, mir.Held) and arg.width == 4 for arg in (*op.args, *op.results))
        assert not op.loads and not op.stores
    for _, body in bodies:
        defined = {value for block in body.blocks for op in block.ops for value in op.defines}
        for block in body.blocks:
            for op in block.ops:
                if op in divisions:
                    assert set(op.uses) <= defined


def test_recovered_memory_arguments_keep_their_relocations() -> None:
    """QuickBASIC chain printed CONST2=0 for 13106 after argument loads read address zero."""
    path = Path("fixtures/regressions/chain-stack-q-O.obj")
    result = wholeseg.emitted(path.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    decoded = ir.decode_module(found)
    assert not isinstance(decoded, str)
    memory = [
        ref
        for body in decoded
        for node in body.nodes
        for ref in (*node.effects.loads, *node.effects.stores)
        if ref.addr is not None
    ]
    assert memory
    assert not any(ref.addr.space is module.Space.LITERAL and ref.addr.disp == 0 for ref in memory)


def test_nested_multiply_consumes_values_without_stealing_outer_arguments() -> None:
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    product = next(op for op in ops if op.at == 0x1cd and op.kind is mir.Kind.MUL)
    division = next(op for op in ops if op.at == 0x1d4 and op.kind is mir.Kind.DIVMOD)
    assert product.node is None and len(product.results) == 1
    assert product.results[0].width == 4
    assert len(product.args) == 2 and all(isinstance(arg, mir.Held) and arg.width == 4 for arg in product.args)
    definitions = {value: op for op in ops for value in op.defines}
    divisor = definitions[division.args[1].value]
    assert divisor.kind is mir.Kind.CONCAT
    assert [definitions[arg.value].at for arg in divisor.args] == [0x1be, 0x1c0]
    assert all(definitions[arg.value].kind is mir.Kind.COPY for arg in divisor.args)
def test_nbody_computed_loop_limit_is_a_native_comparison():
    """NBODY pushed its updated step counter into CPI4 instead of comparing its whole value."""
    path = Path("fixtures/bench/nbody-v-g3.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    comparison = next(op for block in body.blocks for op in block.ops
                      if op.at == 0x2fe and op.op is ir.Operation.COMPARE)
    assert comparison.op is ir.Operation.COMPARE
    assert len(comparison.args) == 2 and all(arg.width == 4 for arg in comparison.args)
    assert comparison.defines and all(value.flags for value in comparison.defines)
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    assert definitions[comparison.args[0].value].kind is mir.Kind.CONCAT
    assert definitions[comparison.args[1].value].at == 0x2f9
    emitted = wholeseg.emitted(path.read_bytes())
    assert emitted.outcome is wholeseg.Emission.LIR, emitted.reason
    assert calls.COMPARE not in module.of(omf.parse(emitted.data)).calls.values()


@pytest.mark.parametrize("flag", [ir.Flag.CF, ir.Flag.PF, ir.Flag.AF])
def test_captured_comparison_retains_runtime_synthesized_flags(monkeypatch, flag):
    """CPI4's CF/PF/AF are not a native CMP's; capturing operands does not change that contract."""
    original = mir._flags_after
    monkeypatch.setattr(mir, "_flags_after", lambda *args: original(*args) | flag)
    path = Path("fixtures/bench/nbody-v-g3.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    assert any(op.at == 0x2fe and op.kind is mir.Kind.CALL for block in body.blocks for op in block.ops)
