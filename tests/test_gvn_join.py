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
                      args=(mir.Held(source, 4), mir.Const(7, 4)), results=(mir.Held(value, 4),))

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


@pytest.mark.parametrize("reverse", [False, True])
def test_missing_path_gets_the_computation(reverse):
    body = diamond()
    body = replace(body, blocks=tuple(replace(block, ops=()) if block.at == 20 else block for block in body.blocks))
    if reverse:
        body = replace(body, blocks=tuple(reversed(body.blocks)))
    after = gvn.joined(body)
    blocks = {block.at: block for block in after.blocks}
    assert not any(op.kind is mir.Kind.MUL for op in blocks[30].ops)
    inserted, = blocks[20].ops
    assert inserted.kind is mir.Kind.MUL and inserted.inserted
    assert blocks[30].phis[0].incoming[20] == inserted.results[0].value
    assert gvn.joined(after) == after


def test_different_operands_cannot_supply_the_join():
    body = diamond()
    right = body.blocks[2]
    product = replace(right.ops[0], args=(right.ops[0].args[0], mir.Const(8, 4)))
    body = replace(body, blocks=(*body.blocks[:2], replace(right, ops=(product,)), body.blocks[-1]))
    after = gvn.joined(body)
    right = next(block for block in after.blocks if block.at == 20)
    assert right.ops[0] == product
    assert right.ops[1].args[-1] == mir.Const(7, 4)
    assert not any(op.kind is mir.Kind.MUL for block in after.blocks if block.at == 30 for op in block.ops)


def test_missing_critical_edge_does_not_speculate():
    body = diamond()
    right = replace(body.blocks[2], ops=(), succ=(30, 40))
    body = replace(body, blocks=(*body.blocks[:2], right, body.blocks[-1], mir.MirBlock(40, (), (), ())))
    assert gvn.joined(body) == body


def test_no_available_path_does_not_distribute_work():
    body = diamond()
    body = replace(body, blocks=tuple(replace(block, ops=()) if block.at in (10, 20) else block for block in body.blocks))
    assert gvn.joined(body) == body


def test_insertion_does_not_clobber_a_join_prefix_condition():
    body = diamond()
    flags = mir.Value(99, 0, flags=True)
    join = body.blocks[-1]
    condition = mir.Op(29, ir.Operation.NOTHING, "", (), (flags,), kind=mir.Kind.OPAQUE)
    body = replace(body, blocks=(*body.blocks[:2], replace(body.blocks[2], ops=()),
                                replace(join, ops=(condition, *join.ops))))
    assert gvn.joined(body) == body


def test_missing_provider_is_inserted_before_the_jump():
    body = diamond()
    jump = mir.Op(22, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP, target=30)
    body = replace(body, blocks=(*body.blocks[:2], replace(body.blocks[2], ops=(jump,)), body.blocks[-1]))
    after = gvn.joined(body)
    assert after.blocks[2].ops[-1] == jump
    assert after.blocks[2].ops[0].kind is mir.Kind.MUL


def test_read_flags_keep_the_join_computation():
    body = diamond()
    joined = body.blocks[-1]
    flags = mir.Value(99, 30, flags=True)
    product = replace(joined.ops[0], defines=(*joined.ops[0].defines, flags))
    use = replace(joined.ops[1], uses=(*joined.ops[1].uses, flags))
    body = replace(body, blocks=(*body.blocks[:-1], replace(joined, ops=(product, use))))
    assert gvn.joined(body) == body


@pytest.mark.parametrize("reverse", [False, True])
def test_join_translates_phi_inputs_on_each_edge(reverse):
    body = diamond()
    entry, left, right, join = body.blocks
    other, selected = mir.Value(2, 1), mir.Value(3, 30)
    define = replace(entry.ops[0], at=1, defines=(other,), results=(mir.Held(other, 4),))
    right_product = replace(right.ops[0], args=(mir.Held(other, 4), mir.Const(7, 4)), uses=(other,))
    joined_product = replace(join.ops[0], args=(mir.Held(selected, 4), mir.Const(7, 4)), uses=(selected,))
    phi = mir.Phi(selected, {10: entry.ops[0].defines[0], 20: other})
    blocks = (replace(entry, ops=(*entry.ops, define)), left, replace(right, ops=(right_product,)),
              replace(join, phis=(phi,), ops=(joined_product, join.ops[1])))
    body = replace(body, blocks=tuple(reversed(blocks)) if reverse else blocks)
    after = gvn.joined(body)
    joined = next(block for block in after.blocks if block.at == 30)
    assert not any(op.kind is mir.Kind.MUL for op in joined.ops)
    assert joined.phis[-1].incoming == {10: left.ops[0].results[0].value, 20: right_product.results[0].value}


def test_phi_translation_is_simultaneous_not_recursive():
    first, second = mir.Value(1, 10), mir.Value(2, 10)
    op = replace(diamond().blocks[-1].ops[0], args=(mir.Held(first, 4), mir.Held(second, 4)), uses=(first, second))
    phis = (mir.Phi(first, {20: second}), mir.Phi(second, {20: first}))
    translated = gvn._on_edge(op, phis, 20)
    assert translated.args == (mir.Held(second, 4), mir.Held(first, 4))
    assert translated.uses == (second, first)
    assert gvn._on_edge(op, phis, 30) is None


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program", ["gvnjn", "gvnphi"])
def test_real_diamond_emits_one_fewer_multiply(tag, program, monkeypatch):
    """GVNJN prints 35,36/37,36; GVNPHI 48,49/37,36. Both repeated the square at the join."""
    from pathlib import Path
    from iced_x86 import Mnemonic
    from qbopt import wholeseg
    from qbopt.frontend import blocks
    from qbopt.objectfile import module, omf

    data = Path(f"fixtures/regressions/{program}-{tag}.obj".lower()).read_bytes()
    with monkeypatch.context() as before:
        before.setattr(gvn, "joined", lambda body, **options: body)
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


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_partial_redundancy_path_skips_the_other_paths_multiply(tag):
    """GVNPRE printed 37,1296 instead of 37,36 when a join label entered the inserted square."""
    from pathlib import Path
    from iced_x86 import Mnemonic
    from qbopt import wholeseg
    from qbopt.frontend import blocks
    from qbopt.objectfile import module, omf

    result = wholeseg.emitted(Path(f"fixtures/regressions/gvnpre-{tag}.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str), mapped
    decoded = [one for block in blocks.partition(found, mapped) for one in block.insns]
    first, second = [one.at for one in decoded if one.insn.mnemonic == Mnemonic.IMUL]
    branch, = [one for one in decoded if first < one.at < second and one.insn.mnemonic == Mnemonic.JMP]
    assert branch.insn.near_branch_target > second
