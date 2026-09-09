from dataclasses import replace

import pytest

from qbopt import ir
from qbopt import mir
from qbopt import loops
from qbopt import strength
from qbopt import induction


def body() -> tuple[mir.MirBody, loops.Loop]:
    start = mir.Value(10, 0, variable=7)
    counter = mir.Value(11, 1, variable=7)
    following = mir.Value(12, 1, variable=7)
    unrelated = mir.Value(13, 1, variable=7)
    answer = mir.Value(14, 1, variable=8)
    step = mir.Op(
        1,
        ir.Operation.UNARY,
        "",
        (following,),
        (counter,),
        kind=mir.Kind.INCREMENT,
        args=(mir.Held(counter, 2),),
        results=(mir.Held(following, 2),),
    )
    multiply = mir.Op(
        2,
        ir.Operation.BINARY,
        "",
        (answer,),
        (unrelated,),
        kind=mir.Kind.MUL,
        args=(mir.Held(unrelated, 2), mir.Const(2, 2)),
        results=(mir.Held(answer, 2),),
    )
    blocks = (
        mir.MirBlock(0, (), (), (1,)),
        mir.MirBlock(1, (mir.Phi(counter, {0: start, 1: following}),), (step, multiply), (1, 2)),
        mir.MirBlock(2, (), (), ()),
    )
    return mir.MirBody(0, blocks), loops.Loop(1, frozenset({1}), frozenset({1}))


def test_a_shared_variable_name_is_not_a_shared_recurrence() -> None:
    """harr's shifted address was mistaken for both loop counters because all shared a variable number."""
    built, loop = body()
    assert induction.basics(built, loop)
    assert not induction.derived(built, loop)
    header = built.blocks[1]
    counter = header.phis[0].result
    multiply = replace(header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Const(2, 2)))
    positive = replace(built, blocks=(built.blocks[0], replace(header, ops=(header.ops[0], multiply)), built.blocks[2]))
    assert len(induction.derived(positive, loop)) == 1


def test_the_backedge_must_step_the_exact_phi_value() -> None:
    built, loop = body()
    header = built.blocks[1]
    unrelated = header.ops[1].args[0]
    step = replace(header.ops[0], args=(unrelated,), uses=(unrelated.value,))
    built = replace(built, blocks=(built.blocks[0], replace(header, ops=(step, header.ops[1])), built.blocks[2]))
    assert not induction.basics(built, loop)


@pytest.mark.parametrize(("width", "count"), [(2, 1), (4, 0)])
def test_only_width_preserving_copies_carry_the_recurrence(width: int, count: int) -> None:
    built, loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    copied = header.ops[1].args[0].value
    copy = mir.Op(
        1,
        ir.Operation.MOVE,
        "",
        (copied,),
        (counter,),
        kind=mir.Kind.COPY,
        args=(mir.Held(counter, width),),
        results=(mir.Held(copied, 2),),
    )
    built = replace(built, blocks=(built.blocks[0], replace(header, ops=(copy, *header.ops)), built.blocks[2]))
    assert len(induction.derived(built, loop)) == count


def test_a_reduced_counter_has_its_own_loop_phi_and_fresh_variable() -> None:
    """harr's experimental stride reused a promoted variable and read its initial value on every iteration."""
    built, _loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    answer = header.ops[1].defines[0]
    multiply = replace(header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Const(2, 2)), covers=(2, 4))
    livein = mir.Value(99, 0, variable=77)
    use = mir.Op(
        4,
        ir.Operation.PUSH,
        "",
        (),
        (answer, livein),
        kind=mir.Kind.ARG,
        args=(mir.Held(answer, 2), mir.Held(livein, 2)),
        covers=(4, 6),
    )
    built = replace(
        built,
        blocks=(
            built.blocks[0],
            replace(header, ops=(replace(header.ops[0], covers=(1, 2)), multiply, use)),
            built.blocks[2],
        ),
    )
    result = strength.reduced(built)
    after = result.blocks[1]
    added = [phi for phi in after.phis if phi.result != counter]
    assert len(added) == 1
    phi = added[0]
    assert phi.result.variable > livein.variable
    assert next(op for op in after.ops if op.kind is mir.Kind.ARG).args[0].value == phi.result
    assert phi.incoming[0] != phi.incoming[1]
    step = next(op for op in after.ops if phi.incoming[1] in op.defines)
    assert step.args[0].value == phi.result
