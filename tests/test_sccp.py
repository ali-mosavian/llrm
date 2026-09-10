"""Executable edges must feed phi facts before later branches are decided."""

from dataclasses import replace

from qbopt.analysis import constant_cycles, consts
from qbopt.model import ir, mir
from qbopt.optimize import transform


def diamond():
    left, right, joined = (mir.Value(index, 0) for index in range(1, 4))

    def copy(at, value, number):
        return mir.Op(at, ir.Operation.MOVE, "mov", (value,), (), kind=mir.Kind.COPY,
                      args=(mir.Const(number, 4),), results=(mir.Held(value, 4),))

    def branch(at, first, second, target):
        flags = mir.Value(at + 100, at, True)
        uses = (first.value,) if isinstance(first, mir.Held) else ()
        compare = mir.Op(at, ir.Operation.COMPARE, "cmp", (flags,), uses, kind=mir.Kind.SUB,
                         args=(first, second), covers=(at, at + 1))
        jump = mir.Op(at + 1, ir.Operation.BRANCH, "", (), (flags,), kind=mir.Kind.BRANCH,
                      test=mir.Kind.EQ, target=target, covers=(at + 1, at + 2))
        return compare, jump

    return mir.MirBody(0, (
        mir.MirBlock(0, (), branch(0, mir.Const(1, 4), mir.Const(1, 4), 10), (10, 20)),
        mir.MirBlock(10, (), (copy(10, left, 7),), (30,)),
        mir.MirBlock(20, (), (copy(20, right, 9),), (30,)),
        mir.MirBlock(30, (mir.Phi(joined, {10: left, 20: right}),),
                     branch(30, mir.Held(joined, 4), mir.Const(7, 4), 40), (40, 50)),
        mir.MirBlock(40, (), (), ()), mir.MirBlock(50, (), (), ()),
    ))


def test_conditional_phi_decides_following_branch_in_one_round():
    result = transform.decided(diamond(), frozenset(), {})
    join = next(block for block in result.blocks if block.at == 30)
    assert join.succ == (40,)
    assert join.ops[-1].kind is mir.Kind.JUMP


def test_runtime_condition_preserves_both_phi_inputs():
    body = diamond()
    entry = body.blocks[0]
    compare = replace(entry.ops[0], args=(mir.Held(mir.Value(999, 0), 4), mir.Const(1, 4)),
                      uses=(mir.Value(999, 0),))
    body = replace(body, blocks=(replace(entry, ops=(compare, entry.ops[1])), *body.blocks[1:]))
    result = transform.decided(body, frozenset(), {})
    assert next(block for block in result.blocks if block.at == 30).succ == (40, 50)


def test_unresolved_successor_callback_cannot_drop_edges():
    body = diamond()
    facts = constant_cycles.propagated(body, {}, lambda block, facts, states: None)
    joined = body.blocks[3].phis[0].result
    assert joined not in facts


def test_new_backedge_invalidates_an_optimistic_loop_constant():
    start, joined, advanced = (mir.Value(index, 0) for index in range(1, 4))
    seed = mir.Op(0, ir.Operation.MOVE, "mov", (start,), (), kind=mir.Kind.COPY,
                  args=(mir.Const(7, 4),), results=(mir.Held(start, 4),))
    update = mir.Op(11, ir.Operation.BINARY, "add", (advanced,), (joined,), kind=mir.Kind.ADD,
                    args=(mir.Held(joined, 4), mir.Const(1, 4)), results=(mir.Held(advanced, 4),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (seed,), (10,)),
                          mir.MirBlock(10, (mir.Phi(joined, {0: start, 10: advanced}),), (update,), (10, 20)),
                          mir.MirBlock(20, (), (), ())))
    visited = []

    def successors(block, facts, states):
        if block.at != 10:
            return block.succ
        visited.append(facts.get(joined))
        if states[joined] is constant_cycles.State.PENDING:
            return None
        return (10,) if facts.get(joined) == consts.Known(7, 4) else block.succ

    facts = constant_cycles.propagated(body, {}, successors)
    assert consts.Known(7, 4) in visited and visited[-1] is None
    assert joined not in facts and advanced not in facts


def test_block_order_does_not_change_conditional_results():
    body = diamond()
    other = replace(body, blocks=tuple(reversed(body.blocks)))
    assert {block.at: block for block in transform.decided(body, frozenset(), {}).blocks} == {
        block.at: block for block in transform.decided(other, frozenset(), {}).blocks}


def test_entry_phi_keeps_unknown_caller_input():
    incoming, joined, returned = (mir.Value(index, 0) for index in range(1, 4))
    copy = mir.Op(10, ir.Operation.MOVE, "mov", (returned,), (), kind=mir.Kind.COPY,
                  args=(mir.Const(7, 4),), results=(mir.Held(returned, 4),))
    body = mir.MirBody(0, (
        mir.MirBlock(0, (mir.Phi(joined, {-1: incoming, 10: returned}),), (), (10,)),
        mir.MirBlock(10, (), (copy,), (0,)),
    ))
    facts = constant_cycles.propagated(body, {}, lambda block, facts, states: block.succ)
    assert joined not in facts
