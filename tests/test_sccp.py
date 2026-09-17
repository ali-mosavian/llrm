"""Executable edges must feed phi facts before later branches are decided."""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import consts
from qbopt.optimize import transform
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space
from qbopt.analysis import constant_cycles
from qbopt.analysis import interprocedural


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


def _returned(number: int, *, at: int = 1) -> mir.MirBody:
    value = mir.Value(at, at, variable=at, version=1)
    copy = mir.Op(
        at,
        ir.Operation.NOTHING,
        "",
        (value,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(number, 2),),
        results=(mir.Held(value, 2),),
    )
    ret = mir.Op(at + 1, ir.Operation.NOTHING, "", (), (value,), kind=mir.Kind.RETURN, args=(mir.Held(value, 2),))
    return mir.MirBody(at, (mir.MirBlock(at, (), (copy, ret), ()),), sealed=True)


def test_module_constant_returns_require_every_exit_to_agree():
    """One convenient return must not become the result of the whole procedure."""
    agrees = _returned(37)
    left, right = agrees.blocks[0], _returned(38, at=10).blocks[0]
    disagrees = replace(agrees, blocks=(replace(left, succ=(10,)), right))
    assert interprocedural.constant_returns({"yes": agrees, "no": disagrees}) == {
        "yes": (mir.Const(37, 2),)
    }


def test_parameter_specialization_requires_every_call_to_agree():
    """One constant call cannot specialize a body also called with another value."""
    seven, nine = mir.Const(7, 2), mir.Const(9, 2)
    procedures = {
        "a": ({1: "leaf"}, {1: (seven,)}),
        "b": ({2: "leaf"}, {2: (nine,)}),
    }
    assert interprocedural.constant_parameters(procedures, frozenset({"leaf"})) == {}
    procedures["b"] = ({2: "leaf"}, {2: (seven,)})
    assert interprocedural.constant_parameters(procedures, frozenset({"leaf"})) == {
        "leaf": (seven,)
    }


def test_pure_call_removal_drops_its_exact_argument_pushes():
    """Deleting a cdecl call must drop its ARG but keep same-site facts.

    Constant return propagation inserts a COPY at the call's source address.
    Deleting every operation with that address left answer_from_argument as a
    bare RETF instead of returning 42.
    """
    argument = mir.Op(1, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.ARG, args=(mir.Const(9, 2),))
    result = mir.Value(2, 2, variable=2, version=1)
    call = mir.Op(
        2,
        ir.Operation.NOTHING,
        "",
        (result,),
        (),
        kind=mir.Kind.CALL,
        results=(mir.Held(result, 2),),
    )
    answer = mir.Value(3, 2, variable=3, version=1)
    constant = mir.Op(
        2,
        ir.Operation.NOTHING,
        "",
        (answer,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(42, 2),),
        results=(mir.Held(answer, 2),),
    )
    ret = mir.Op(3, ir.Operation.NOTHING, "", (), (answer,), kind=mir.Kind.RETURN, args=(mir.Held(answer, 2),))
    body = mir.MirBody(1, (mir.MirBlock(1, (), (argument, call, constant, ret), ()),), sealed=True)

    class Contract:
        cleanup = 0
        caller_cleanup = 2

    sites = interprocedural.argument_sites(body, {2: Contract()})
    made = interprocedural.remove_dead_pure_calls(body, {2: "leaf"}, frozenset({"leaf"}), sites)
    assert [op.kind for op in made.blocks[0].ops] == [mir.Kind.COPY, mir.Kind.RETURN]


def test_purity_refuses_nontermination_and_nonlocal_stores():
    """A constant return does not license deleting a loop or a global write."""
    looping = mir.MirBody(1, (mir.MirBlock(1, (), (), (1,)),), sealed=True)
    global_ = mir.MemRef(Addr(Space.SEGMENT, 0, 1), 2, space=Space.SEGMENT)
    store = mir.Op(
        1,
        ir.Operation.NOTHING,
        "",
        (),
        (),
        kind=mir.Kind.STORE,
        args=(mir.Const(1, 2),),
        results=(mir.Cell(global_),),
        stores=(global_,),
    )
    returned = _returned(1).blocks[0].ops[-1]
    writing = mir.MirBody(1, (mir.MirBlock(1, (), (store, returned), ()),), sealed=True)
    assert interprocedural.pure_procedures({"loop": (looping, {}), "write": (writing, {})}) == frozenset()
