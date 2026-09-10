"""A diamond recomputed a multiply even though both incoming paths already computed it."""

from dataclasses import replace

import pytest

from qbopt.model import ir, mir
from qbopt.optimize import gvn


def diamond():
    source = mir.Value(1, 0)
    define = mir.Op(0, ir.Operation.MOVE, "", (source,), (), kind=mir.Kind.LOAD,
                    args=(mir.Cell(mir.MemRef(None, 4)),), results=(mir.Held(source, 4),))

    def product(at):
        value = mir.Value(at, at)
        return mir.Op(at, ir.Operation.MULTIPLY, "imul", (value,), (source,), kind=mir.Kind.MUL,
                      args=(mir.Held(source, 4), mir.Const(7, 4)), results=(mir.Held(value, 4),),
                      covers=(at, at + 2))

    left, right, merged = map(product, (10, 20, 30))
    use = mir.Op(32, ir.Operation.PUSH, "push", (), merged.defines, kind=mir.Kind.ARG, args=merged.results)
    return mir.MirBody(0, (mir.MirBlock(0, (), (define,), (10, 20)),
                           mir.MirBlock(10, (), (left,), (30,)),
                           mir.MirBlock(20, (), (right,), (30,)),
                           mir.MirBlock(30, (), (merged, use), ())))


@pytest.mark.parametrize("reverse", [False, True])
def test_join_reuses_each_incoming_product(reverse):
    body = diamond()
    if reverse:
        body = replace(body, blocks=tuple(reversed(body.blocks)))
    after = gvn.joined(body)
    blocks = {block.at: block for block in after.blocks}
    joined = blocks[30]
    assert not any(op.kind is mir.Kind.MUL for op in joined.ops)
    assert joined.phis == (mir.Phi(joined.ops[-1].args[0].value,
                                  {at: blocks[at].ops[0].results[0].value for at in (10, 20)}),)
    assert joined.phis[0].result.at == joined.at
    assert gvn.joined(after) == after


def test_missing_path_keeps_the_computation():
    body = diamond()
    body = replace(body, blocks=tuple(replace(block, ops=()) if block.at == 20 else block for block in body.blocks))
    assert gvn.joined(body) == body


def test_different_operands_cannot_supply_the_join():
    body = diamond()
    right = body.blocks[2]
    product = replace(right.ops[0], args=(right.ops[0].args[0], mir.Const(8, 4)))
    body = replace(body, blocks=(*body.blocks[:2], replace(right, ops=(product,)), body.blocks[-1]))
    assert gvn.joined(body) == body


def test_read_flags_keep_the_join_computation():
    body = diamond()
    joined = body.blocks[-1]
    flags = mir.Value(99, 30, flags=True)
    product = replace(joined.ops[0], defines=(*joined.ops[0].defines, flags))
    use = replace(joined.ops[1], uses=(*joined.ops[1].uses, flags))
    body = replace(body, blocks=(*body.blocks[:-1], replace(joined, ops=(product, use))))
    assert gvn.joined(body) == body


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_real_diamond_emits_one_fewer_multiply(tag, monkeypatch):
    """GVNJN prints 35,36 and 37,36; both branches used to multiply again at the join."""
    from pathlib import Path
    from iced_x86 import Mnemonic
    from qbopt import wholeseg
    from qbopt.frontend import blocks
    from qbopt.objectfile import module, omf

    data = Path(f"fixtures/regressions/gvnjn-{tag}.obj").read_bytes()
    with monkeypatch.context() as before:
        before.setattr(gvn, "joined", lambda body: body)
        old = wholeseg.emitted(data)
    new = wholeseg.emitted(data)

    def multiplies(result):
        assert result.outcome is wholeseg.Emission.LIR, result.reason
        found = module.of(omf.parse(result.data))
        mapped = blocks.code_map(found)
        assert not isinstance(mapped, str), mapped
        return sum(one.insn.mnemonic == Mnemonic.IMUL
                   for block in blocks.partition(found, mapped) for one in block.insns)

    assert multiplies(old) == 3
    assert multiplies(new) == 2
