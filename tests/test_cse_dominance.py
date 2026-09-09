"""Value-numbering candidates are scoped by dominance, not visitation order."""

from dataclasses import replace

import pytest

from qbopt.model import ir, mir
from qbopt.optimize import transform


@pytest.mark.parametrize("reverse", [False, True])
def test_sibling_computation_does_not_hide_local_reuse(reverse):
    """A diamond's second visited arm retained a duplicate addition unnecessarily."""
    source = mir.Value(1, 0, variable=1)
    define = mir.Op(0, ir.Operation.MOVE, "mov", (source,), (), kind=mir.Kind.COPY,
                    args=(mir.Const(3, 2),), results=(mir.Held(source, 2),), covers=(0, 2))

    def arm(at):
        first = mir.Value(at, at, variable=at)
        second = mir.Value(at + 1, at + 2, variable=at + 1)
        ops = tuple(mir.Op(position, ir.Operation.BINARY, "add", (value,), (source,),
                           kind=mir.Kind.ADD, args=(mir.Held(source, 2), mir.Const(7, 2)),
                           results=(mir.Held(value, 2),), covers=(position, position + 2))
                    for position, value in ((at, first), (at + 2, second)))
        use = mir.Op(at + 4, ir.Operation.PUSH, "push", (), (second,), kind=mir.Kind.ARG,
                     args=(mir.Held(second, 2),))
        return mir.MirBlock(at, (), (*ops, use), (40,)), first

    left, left_value = arm(10)
    right, right_value = arm(20)
    join, join_value = arm(40)
    join = replace(join, succ=())
    arms = (right, left) if reverse else (left, right)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (define,), (10, 20)), *arms,
                           join))
    after = transform.subexpressions(body)
    by_at = {block.at: block for block in after.blocks}
    for at, expected in ((10, left_value), (20, right_value), (40, join_value)):
        assert by_at[at].ops[-1].args == (mir.Held(expected, 2),)
        assert sum(op.kind is mir.Kind.ADD for op in by_at[at].ops) == 1
