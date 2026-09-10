"""Dead BOOLS computations left fallthrough-only blocks in the live CFG."""

from dataclasses import replace
from pathlib import Path

import pytest

from qbopt.model import ir, mir
from qbopt.optimize import transform


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_bools_live_cfg_has_no_empty_transit_blocks(tag):
    """BOOLS retained deleted IF arms between its final x=-1 and t=2 stores."""
    from qbopt import wholeseg
    states = []

    def watch(stage, name, body):
        if isinstance(body, mir.MirBody):
            states.append(body)

    result = wholeseg.emitted(Path(f"fixtures/omf/bools-{tag}.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    body = states[-1]
    reached, pending = set(), [body.entry]
    while pending:
        at = pending.pop()
        if at in reached:
            continue
        reached.add(at)
        block = body.block(at)
        pending.extend(block.succ)
        if at != body.entry and len(block.succ) == 1 and not block.phis:
            assert any(op.kind not in {mir.Kind.NOTHING, mir.Kind.JUMP} for op in block.ops)


@pytest.mark.parametrize("explicit", [False, True])
@pytest.mark.parametrize("marker", [False, True])
def test_edges_bypass_empty_fallthrough_blocks(explicit, marker):
    """BOOLS's deleted conditions must not keep empty nodes on live paths."""
    jump = mir.Op(0, ir.Operation.JUMP, "jmp", (), (), kind=mir.Kind.JUMP,
                  target=10, covers=(0, 2))
    empty = mir.Op(10, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.NOTHING,
                   covers=(10, 20))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (jump,) if explicit else (), (10,)),
                          mir.MirBlock(10, (), (empty,) if marker else (), (20,)),
                          mir.MirBlock(20, (), (), ())))
    result = transform._threaded(body)
    assert result.blocks[0].succ == (20,)
    if explicit:
        assert result.blocks[0].ops[-1].target == 20
    assert result.blocks[1].succ == ()
    if marker:
        assert result.blocks[1].ops[0].covers == (10, 20)


@pytest.mark.parametrize("guard", ["phi", "cycle", "input", "checkpoint"])
def test_empty_looking_blocks_with_dependencies_are_not_bypassed(guard):
    value = mir.Value(1, 10)
    empty = mir.Op(10, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.NOTHING)
    end = mir.MirBlock(20, (), (), ())
    if guard == "phi":
        end = replace(end, phis=(mir.Phi(value, {10: mir.Value(2, 0)}),))
    if guard == "input":
        empty = replace(empty, uses=(value,))
    if guard == "checkpoint":
        empty = replace(empty, kind=mir.Kind.FCHECK)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (), (10,)),
                          mir.MirBlock(10, (), (empty,), (10 if guard == "cycle" else 20,)), end))
    assert transform._threaded(body) == body


def test_converged_branch_keeps_its_destination_even_when_condition_is_false():
    """Both empty arms reach PRINT; folding false must not make PRINT unreachable."""
    flags = mir.Value(1, 0, True)
    compare = mir.Op(0, ir.Operation.COMPARE, "cmp", (flags,), (), kind=mir.Kind.SUB,
                     args=(mir.Const(0, 2), mir.Const(1, 2)), covers=(0, 2))
    branch = mir.Op(2, ir.Operation.BRANCH, "", (), (flags,), kind=mir.Kind.BRANCH,
                    test=mir.Kind.EQ, target=10, covers=(2, 4))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (compare, branch), (10, 20)),
                          mir.MirBlock(10, (), (), (30,)),
                          mir.MirBlock(20, (), (), (30,)),
                          mir.MirBlock(30, (), (), ())))
    result = transform.decided(body, frozenset(), {})
    assert result.blocks[0].succ == (30,)
    assert result.blocks[0].ops[-1].kind is mir.Kind.JUMP
    assert not result.blocks[0].ops[-1].uses
