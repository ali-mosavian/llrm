"""A branch must consume its own comparison, not inserted arithmetic's flags."""

from dataclasses import replace

import pytest

from qbopt import ir
from qbopt import mir
from qbopt import lower


def body() -> mir.MirBody:
    value = mir.Value(1, 0)
    condition = mir.Value(2, 0, flags=True)
    updated = mir.Value(3, 0)
    compare = mir.Op(
        0,
        ir.Operation.COMPARE,
        "cmp",
        (condition,),
        (value,),
        kind=mir.Kind.SUB,
        args=(mir.Held(value, 2), mir.Const(19, 2)),
    )
    increment = mir.Op(
        3,
        ir.Operation.BINARY,
        "",
        (updated,),
        (value,),
        kind=mir.Kind.ADD,
        args=(mir.Held(value, 2), mir.Const(42, 2)),
        results=(mir.Held(updated, 2),),
    )
    branch = mir.Op(6, ir.Operation.BRANCH, "jle", (), (condition,), kind=mir.Kind.BRANCH, target=0)
    return mir.MirBody(0, (mir.MirBlock(0, (), (compare, increment, branch), (0,)),))


def test_inserted_stride_cannot_replace_the_branch_condition() -> None:
    """A stride add between cmp and jle branches on the stride's flags instead of the loop bound."""
    built = body()
    result = lower.lowered("loop", built, {}, (), {})
    assert [one.op.at for one in result.blocks[0].insns] == [3, 0, 6]
    assert [one.at for one in built.blocks[0].ops] == [0, 3, 6]


@pytest.mark.parametrize("reason", ["memory", "second_reader", "data_result"])
def test_condition_scheduling_does_not_move_effects_or_other_results(reason: str) -> None:
    built = body()
    compare, increment, branch = built.blocks[0].ops
    if reason == "memory":
        from qbopt.module import Addr
        from qbopt.module import Space

        cell = mir.MemRef(Addr(Space.SEGMENT, 0, 1), 2)
        compare = replace(compare, loads=(cell,), args=(mir.Cell(cell), mir.Const(19, 2)))
    elif reason == "second_reader":
        increment = replace(increment, uses=(*increment.uses, compare.defines[0]))
    else:
        compare = replace(compare, defines=(*compare.defines, mir.Value(4, 0)))
    built = replace(built, blocks=(replace(built.blocks[0], ops=(compare, increment, branch)),))
    with pytest.raises(lower.Unlowered, match="crosses a live condition"):
        lower.lowered("loop", built, {}, (), {})
