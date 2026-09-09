from pathlib import Path

import pytest

import corpus
from qbopt import ir, mir, module, omf, wholeseg


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
