from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt import asm, ir, mir, module, omf, pairs, transform, wholeseg


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
    high = next(op for op in ops if op.at == 0x1cd and op.kind is mir.Kind.CONCAT and op.args[0].value.at == 0x131).args[0].value
    assert any(high in op.defines for op in ops)


def test_divide_relocation_survives_index_value_replacement() -> None:
    """Nbody refused its velocity divide after LICM renamed an index without changing its relocation."""
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    op = next(op for block in body.blocks for op in block.ops if op.at == 0x25f and op.kind is mir.Kind.DIVMOD)
    fields = frozenset(one.offset for one in omf.fixups(found.records) if one.seg == found.seg)
    expected = asm._divide_fields(op, found, fields)
    assert expected
    cell = op.args[0]
    moved = replace(cell, ref=replace(cell.ref, base=mir.Value(99999, 0)))
    renamed = replace(op, args=(moved, *op.args[1:]))
    assert asm._divide_fields(renamed, found, fields) == expected
    different = replace(moved, ref=replace(moved.ref, addr=moved.ref.addr.plus(4)))
    assert asm._divide_fields(replace(op, args=(different, *op.args[1:])), found, fields) is None


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
