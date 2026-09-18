"""Executable edges must feed phi facts before later branches are decided."""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model import memory
from qbopt.analysis import consts
from qbopt.optimize import transform
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space
from qbopt.analysis import constant_cycles
from qbopt.analysis import interprocedural


def test_unreachable_floating_work_becomes_a_complete_inert_marker() -> None:
    """Nbody peeling left x87 semantics on ``nothing`` and failed before lowering."""
    from qbopt.backend import lower_floats
    from qbopt.model.floating import Format
    from qbopt.model.floating import Rounding
    from qbopt.model.floating import Precision
    from qbopt.model.floating import Semantics

    flags = mir.Value(1, 0, flags=True)
    compare = mir.Op(
        1,
        ir.Operation.COMPARE,
        "cmp",
        (flags,),
        (),
        kind=mir.Kind.SUB,
        args=(mir.Const(0, 2), mir.Const(1, 2)),
    )
    branch = mir.Op(
        2,
        ir.Operation.BRANCH,
        "jne",
        (),
        (flags,),
        kind=mir.Kind.BRANCH,
        test=mir.Kind.EQ,
        target=10,
    )
    rule = Semantics((Format.BINARY64,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE)
    floating = mir.Op(
        10,
        ir.Operation.FLOAT_LOAD,
        "fld",
        (),
        (),
        kind=mir.Kind.FLOAD,
        floating=rule,
        absorbed=(10,),
    )
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (compare, branch), (10, 20)),
            mir.MirBlock(10, (), (floating,), (20,)),
            mir.MirBlock(20, (), (), ()),
        ),
    )

    changed = transform.decided(body, frozenset(), {})

    marker = changed.block(10).ops[0]
    assert marker.kind is mir.Kind.NOTHING
    assert marker.absorbed == (10,)
    assert marker.floating is None
    lower_floats.checked(changed)


def diamond():
    left, right, joined = (mir.Value(index, 0) for index in range(1, 4))

    def copy(at, value, number):
        return mir.Op(at, ir.Operation.MOVE, "mov", (value,), (), kind=mir.Kind.COPY,
                      args=(mir.Const(number, 4),), results=(mir.Held(value, 4),))

    def branch(at, first, second, target):
        flags = mir.Value(at + 100, at, True)
        uses = (first.value,) if isinstance(first, mir.Held) else ()
        compare = mir.Op(at, ir.Operation.COMPARE, "cmp", (flags,), uses, kind=mir.Kind.SUB,
                         args=(first, second))
        jump = mir.Op(at + 1, ir.Operation.BRANCH, "", (), (flags,), kind=mir.Kind.BRANCH,
                      test=mir.Kind.EQ, target=target)
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


def test_nullable_pointer_parameter_keeps_both_null_test_edges():
    """Object addresses are non-null, but an incoming pointer still may be null."""
    pointer = mir.Value(1, 0, variable=1, version=1)
    flags = mir.Value(2, 0, flags=True, variable=2, version=1)
    compare = mir.Op(
        1,
        ir.Operation.COMPARE,
        "cmp",
        (flags,),
        (pointer,),
        kind=mir.Kind.SUB,
        args=(mir.Held(pointer, 2), mir.Const(0, 2)),
    )
    branch = mir.Op(
        2,
        ir.Operation.BRANCH,
        "",
        (),
        (flags,),
        kind=mir.Kind.BRANCH,
        test=mir.Kind.EQ,
        target=10,
    )
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (compare, branch), (10, 20)),
            mir.MirBlock(10, (), (), ()),
            mir.MirBlock(20, (), (), ()),
        ),
        pointer_values=frozenset({pointer}),
        pointer_seeds={pointer: memory.Provenance.one(memory.Object(memory.Kind.PARAMETER, 0))},
    )

    decided = transform.decided(body, frozenset(), {})

    entry = decided.block(0)
    assert entry is not None
    assert entry.succ == (10, 20)
    assert entry.ops[-1].kind is mir.Kind.BRANCH


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


def test_current_parameter_constants_reads_a_sccp_returned_actual():
    """A prior private return made choose's actual 4 after raising, not in source facts."""
    value = mir.Value(1, 1, variable=1, version=1)
    argument = mir.Op(
        1,
        ir.Operation.NOTHING,
        "",
        (),
        (),
        kind=mir.Kind.ARG,
        args=(mir.Held(value, 2),),
    )
    call = mir.Op(2, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.CALL)
    body = mir.MirBody(
        1,
        (mir.MirBlock(1, (), (argument, call), ()),),
        initial=(),
        sealed=True,
    )
    # The copy is deliberately inserted before the argument: it models the
    # result materialized from seed() by interprocedural return propagation.
    materialized = mir.Op(
        0,
        ir.Operation.NOTHING,
        "",
        (value,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(4, 2),),
        results=(mir.Held(value, 2),),
    )
    body = replace(body, blocks=(replace(body.blocks[0], ops=(materialized, argument, call)),))
    parameter = mir.MemRef(Addr(Space.FRAME, 0, 1), 2, space=Space.FRAME)
    assert interprocedural.current_parameter_constants(
        {"caller": body},
        {"caller": {2: "choose"}},
        {"caller": {2: frozenset({1})}},
        {"choose": (parameter,)},
        frozenset({"choose"}),
    ) == {"choose": (mir.Const(4, 2),)}


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
