from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.analysis import loops
from qbopt.backend import select
from qbopt.backend import allocate
from qbopt.optimize import transform
from qbopt.backend import lower_switches


def switched() -> mir.MirBody:
    selector = mir.Value(1, 0, variable=1, version=1)
    merged = mir.Value(2, 20, variable=2, version=1)
    op = mir.Op(
        5,
        ir.Operation.JUMP,
        "",
        (),
        (selector,),
        kind=mir.Kind.SWITCH,
        args=(mir.Held(selector, 2),),
        target=30,
        cases=((1, 20), (2, 20), (3, 30)),
        covers=(5, 12),
    )
    return mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (op,), (20, 30)),
            mir.MirBlock(20, (mir.Phi(merged, {0: selector}),), (), ()),
            mir.MirBlock(30, (), (), ()),
        ),
    )


def destination(body: mir.MirBody, number: int) -> int:
    block = body.block(body.entry)
    assert block is not None
    while block.ops:
        compare, branch = block.ops[-2:]
        constant = compare.args[1]
        assert isinstance(constant, mir.Const)
        selected = (
            branch.target
            if number == constant.n or len(block.succ) == 1
            else next(at for at in block.succ if at != branch.target)
        )
        assert selected is not None
        block = body.block(selected)
        assert block is not None
    return block.at


def test_switch_expansion_keeps_cases_default_and_shared_destination_phis() -> None:
    body = lower_switches.expanded(switched())
    assert [destination(body, value) for value in (0, 1, 2, 3, 4, 255)] == [30, 20, 20, 30, 30, 30]
    predecessors = loops.predecessors(body.blocks)
    target = body.block(20)
    assert target is not None
    assert set(target.phis[0].incoming) == set(predecessors[20])
    assert len(target.phis[0].incoming) == 2
    assert sum(op.covers == (5, 12) for block in body.blocks for op in block.ops) == 1


def test_switch_is_observable_to_dead_code_elimination() -> None:
    body = transform.dead(switched())
    assert body.blocks[0].ops[-1].kind is mir.Kind.SWITCH


def test_switch_comparisons_cannot_overwrite_a_live_condition() -> None:
    body = switched()
    flags = mir.Value(9, 0, flags=True)
    branch = mir.Op(
        20, ir.Operation.BRANCH, "", (), (flags,), kind=mir.Kind.BRANCH, test=mir.Kind.EQ, target=30, covers=(20, 20)
    )
    body = replace(body, blocks=(body.blocks[0], replace(body.blocks[1], ops=(branch,), succ=(30,)), body.blocks[2]))
    with pytest.raises(lower.Unlowered, match="live condition"):
        lower.lowered("condition", body, {}, set(), {})


def test_lowering_consumes_switches_as_compare_and_branch_operations() -> None:
    body = lower.lowered("switch", switched(), {}, set(), {})
    operations = [insn.what for block in body.blocks for insn in block.insns]
    assert sum(op is not None and op.name == "cmp" for op in operations) == 2
    assert sum(op is not None and op.name == "je" for op in operations) == 2


def test_lowered_switch_comparisons_encode_after_allocation() -> None:
    body = lower.lowered("switch", switched(), {}, set(), {})
    assignment = allocate.allocate(body, {1: Register.EAX})
    assert not assignment.spilled
    body = allocate.applied(body, assignment)
    encoded = []
    for insn in body.insns:
        if insn.what is not None and insn.what.name == "cmp":
            result = select.emit(insn.what)
            assert result is not None
            encoded.append(result.code)
    assert encoded == [bytes.fromhex(code) for code in ("83f801", "83f802")]


@pytest.mark.parametrize(("value", "target"), [(1, 20), (2, 20), (3, 30), (0, 30), (65537, 20)])
def test_constant_switch_emits_only_a_jump(value: int, target: int) -> None:
    body = switched()
    source = body.blocks[0]
    op = replace(source.ops[0], args=(mir.Const(value, 2),), uses=())
    body = replace(body, blocks=(replace(source, ops=(op,)), *body.blocks[1:]))
    lowered = lower.lowered("constant", body, {}, set(), {})
    (jump,) = lowered.blocks[0].insns
    assert jump.what is not None
    assert jump.what.name == "jmp"
    assert lowered.blocks[0].succ == (target,)


@pytest.mark.parametrize(("value", "target"), [(1, 20), (0, 30), (65537, 20)])
def test_sccp_resolves_semantic_switches(value: int, target: int) -> None:
    body = switched()
    source = body.blocks[0]
    op = replace(source.ops[0], args=(mir.Const(value, 2),), uses=())
    body = replace(body, blocks=(replace(source, ops=(op,)), *body.blocks[1:]))
    changed = transform.decided(body, frozenset(), {})
    assert changed.blocks[0].succ == (target,)
    assert changed.blocks[0].ops[-1].kind is mir.Kind.JUMP


@pytest.mark.parametrize("invalid", ["duplicate", "missing", "effects", "successors"])
def test_invalid_switch_is_rejected_atomically(invalid: str) -> None:
    body = switched()
    block = body.blocks[0]
    op = block.ops[0]
    match invalid:
        case "duplicate":
            op = replace(op, cases=((1, 20), (65537, 30)))
        case "missing":
            op = replace(op, target=40)
        case "effects":
            op = replace(op, defines=(mir.Value(9, 5),))
        case "successors":
            block = replace(block, succ=(20,))
    changed = replace(body, blocks=(replace(block, ops=(op,)), *body.blocks[1:]))
    with pytest.raises(lower.Unlowered):
        lower.lowered("invalid", changed, {}, set(), {})
    assert body == switched()
