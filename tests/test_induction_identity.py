from dataclasses import replace

from qbopt import ir
from qbopt import mir
from qbopt import loops
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
