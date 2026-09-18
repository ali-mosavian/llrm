"""Exact nested recurrence rewind belongs in the bounded Tier 1 loop."""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import loops


def test_exact_nested_recurrence_rewinds_before_reloading_its_start() -> None:
    """Mandel reloaded and stored ``xStart`` at the beginning of every row.

    The inner coordinate has advanced by exactly ``32 * 24`` on its only
    exit.  On targets where a memory update is no dearer than a reload plus
    store, carrying that completed value around the outer backedge and
    subtracting the proven distance removes the saved-start live range.  A
    386 must retain the copy because its memory arithmetic is slower.
    """
    from qbopt.analysis import consts
    from qbopt.optimize import indvars
    from qbopt.analysis import induction
    from qbopt.model.passes import OperationCosts

    def value(serial: int, at: int, variable: int) -> mir.Value:
        return mir.Value(serial, at, variable=variable)

    def copy(at: int, result: mir.Value, source: mir.Arg) -> mir.Op:
        uses = (source.value,) if isinstance(source, mir.Held) else ()
        return mir.Op(
            at,
            ir.Operation.NOTHING,
            "",
            (result,),
            uses,
            kind=mir.Kind.COPY,
            args=(source,),
            results=(mir.Held(result, source.width),),
        )

    def add(at: int, result: mir.Value, source: mir.Value, amount: int, width: int = 4) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.NOTHING,
            "",
            (result,),
            (source,),
            kind=mir.Kind.ADD,
            args=(mir.Held(source, width), mir.Const(amount, width)),
            results=(mir.Held(result, width),),
        )

    def compare(at: int, source: mir.Value, bound: int | mir.Value, width: int) -> tuple[mir.Op, mir.Value]:
        flag = mir.Value(100 + at, at, flags=True, variable=100 + at)
        operand = mir.Held(bound, width) if isinstance(bound, mir.Value) else mir.Const(bound, width)
        return (
            mir.Op(
                at,
                ir.Operation.NOTHING,
                "",
                (flag,),
                (source, bound) if isinstance(bound, mir.Value) else (source,),
                kind=mir.Kind.SUB,
                args=(mir.Held(source, width), operand),
                results=(),
            ),
            flag,
        )

    def branch(at: int, flag: mir.Value, test: mir.Kind, target: int) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.NOTHING,
            "",
            (),
            (flag,),
            kind=mir.Kind.BRANCH,
            test=test,
            target=target,
        )

    argument = value(0, 0, 0)
    start = value(1, 0, 1)
    bound = value(7, 0, 7)
    outer_start = value(2, 0, 2)
    outer = value(3, 1, 2)
    outer_next = value(4, 5, 2)
    current = value(5, 3, 3)
    following = value(6, 4, 3)
    inner_test, inner_flag = compare(3, current, bound, 4)
    outer_test, outer_flag = compare(5, outer_next, 2, 2)
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(
                0,
                (),
                (
                    add(0, start, argument, 5),
                    add(0, bound, start, 8),
                    copy(0, outer_start, mir.Const(0, 2)),
                ),
                (1,),
            ),
            mir.MirBlock(1, (mir.Phi(outer, {0: outer_start, 5: outer_next}),), (), (2,)),
            mir.MirBlock(2, (), (), (3,)),
            mir.MirBlock(
                3,
                (mir.Phi(current, {2: start, 4: following}),),
                (inner_test, branch(3, inner_flag, mir.Kind.EQ, 5)),
                (4, 5),
            ),
            mir.MirBlock(4, (), (add(4, following, current, 2),), (3,)),
            mir.MirBlock(
                5,
                (),
                (add(5, outer_next, outer, 1, 2), outer_test, branch(5, outer_flag, mir.Kind.LT, 1)),
                (1, 6),
            ),
            mir.MirBlock(6, (), (), ()),
        ),
    )
    later_core = OperationCosts(add=1, move=1, load=1, store=1, memory_update=1)
    i386 = OperationCosts(add=2, move=2, load=4, store=2, memory_update=8)

    changed = indvars.rewound(body, registers=1, costs=later_core)
    outer_header = next(block for block in changed.blocks if block.at == 1)
    inner_header = next(block for block in changed.blocks if block.at == 3)
    latch = next(block for block in changed.blocks if block.at == 5)

    assert changed != body
    assert dict(changed.loop_trip_counts) == {3: 4}
    changed_inner = next(loop for loop in loops.loops(changed.blocks, changed.entry) if loop.header == 3)
    assert induction.trip_count(changed, changed_inner, consts.known(changed)) == 4
    from qbopt.optimize import rotate

    assert dict(rotate.rotated(changed).loop_trip_counts) == {4: 4}
    assert len(outer_header.phis) == 2
    assert inner_header.phis[0].incoming[2] != start
    assert any(current in phi.incoming.values() for phi in latch.phis)
    assert any(
        op.kind is mir.Kind.ADD
        and mir.Const(-8 & 0xFFFFFFFF, 4) in op.args
        and any(value.variable == following.variable for value in op.uses)
        for op in latch.ops
    )
    assert indvars.rewound(body, registers=1, costs=i386) is body

    # The first production version rewound Mandel's rematerializable ``px =
    # -16`` control before the coordinate recurrence existed.  That blocked
    # strength reduction and grew P6 from 199/55/88 to 219/62/95.
    constant_entry = replace(
        body.blocks[0],
        ops=(copy(0, start, mir.Const(5, 4)), *body.blocks[0].ops[1:]),
    )
    constant_body = replace(body, blocks=(constant_entry, *body.blocks[1:]))
    assert indvars.rewound(constant_body, registers=1, costs=later_core) is constant_body
