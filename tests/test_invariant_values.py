"""Fast LICM regressions for complete values that initialize nested loops."""

from qbopt.model import ir
from qbopt.model import mir
from qbopt.optimize import algebraic
from qbopt.optimize import transform


def test_mandel_computed_nested_initial_value_is_loop_invariant() -> None:
    """Mandelbrot recomputed ``xOffset - 512`` once for every image row.

    The value initializes the nested column recurrence, but it is a complete
    pure result of outer-invariant operands.  Hoisting gives it a fresh SSA
    variable; unlike moving a literal counter reset, no later nested update
    can become the next outer iteration's initial value.
    """
    argument = mir.Value(1, 0, variable=1, version=1)
    start = mir.Value(2, 10, variable=2, version=1)
    carried = mir.Value(3, 20, variable=2, version=2)
    advanced = mir.Value(4, 30, variable=2, version=3)
    initial = mir.Op(
        10,
        ir.Operation.BINARY,
        "add",
        (start,),
        (argument,),
        kind=mir.Kind.ADD,
        args=(mir.Held(argument, 4), mir.Const(-512, 4)),
        results=(mir.Held(start, 4),),
    )
    step = mir.Op(
        30,
        ir.Operation.BINARY,
        "add",
        (advanced,),
        (carried,),
        kind=mir.Kind.ADD,
        args=(mir.Held(carried, 4), mir.Const(24, 4)),
        results=(mir.Held(advanced, 4),),
    )
    phi = mir.Phi(carried, {0: start, 30: advanced})

    run = transform._invariant_run(
        [initial, step],
        {carried},
        [],
        frozenset(),
        {},
        [phi],
        starts=transform._starts([phi]),
        readable={start, carried, advanced},
    )

    assert initial in run
    assert step not in run


def test_loop_recurrence_addition_is_independent_of_frontend_association() -> None:
    """Frontend-parity LOOP emitted an extra accumulator move for BASIC.

    BASIC raised ``(product + total) + 3`` while C raised
    ``total + (product + 3)``.  The back-edge phi gives TOTAL the highest
    associative rank, so both forms must leave it at the root update.
    """
    initial = mir.Value(1, 1)
    total = mir.Value(2, 2)
    product = mir.Value(3, 3)
    middle = mir.Value(4, 3)
    updated = mir.Value(5, 3)
    phi = mir.Phi(total, {1: initial, 3: updated})
    multiply = mir.Op(
        3,
        ir.Operation.MULTIPLY,
        "imul",
        (product,),
        (),
        kind=mir.Kind.MUL,
        args=(mir.Const(7, 4), mir.Const(9, 4)),
        results=(mir.Held(product, 4),),
    )
    first = mir.Op(
        3,
        ir.Operation.BINARY,
        "add",
        (middle,),
        (product, total),
        kind=mir.Kind.ADD,
        args=(mir.Held(product, 4), mir.Held(total, 4)),
        results=(mir.Held(middle, 4),),
    )
    last = mir.Op(
        3,
        ir.Operation.BINARY,
        "add",
        (updated,),
        (middle,),
        kind=mir.Kind.ADD,
        args=(mir.Held(middle, 4), mir.Const(3, 4)),
        results=(mir.Held(updated, 4),),
    )
    body = mir.MirBody(
        1,
        (
            mir.MirBlock(1, (), (), (2,)),
            mir.MirBlock(2, (phi,), (), (3, 4)),
            mir.MirBlock(3, (), (multiply, first, last), (2,)),
            mir.MirBlock(4, (), (), ()),
        ),
    )

    result = algebraic._reassociated_recurrences(body)
    inner, outer = result.blocks[2].ops[-2:]

    assert inner.args == (mir.Held(product, 4), mir.Const(3, 4))
    assert outer.args == (mir.Held(total, 4), mir.Held(middle, 4))


def test_zero_test_reuses_the_flags_of_its_value_producer() -> None:
    """PARITYCONTROL emitted ``and si,1 / or si,si / jne`` for BASIC.

    The idempotent OR computes no new value, and AND already defines the
    same zero/nonzero condition.  Equivalent frontend condition spellings
    must therefore converge before lowering.
    """
    source = mir.Value(10, 0)
    masked = mir.Value(11, 1)
    tested = mir.Value(12, 2)
    produced_flags = mir.Value(13, 1, flags=True)
    tested_flags = mir.Value(14, 2, flags=True)
    mask = mir.Op(
        1,
        ir.Operation.BINARY,
        "and",
        (masked, produced_flags),
        (source,),
        kind=mir.Kind.AND,
        args=(mir.Held(source, 2), mir.Const(1, 2)),
        results=(mir.Held(masked, 2),),
    )
    zero_test = mir.Op(
        2,
        ir.Operation.BINARY,
        "or",
        (tested, tested_flags),
        (masked,),
        kind=mir.Kind.OR,
        args=(mir.Held(masked, 2), mir.Held(masked, 2)),
        results=(mir.Held(tested, 2),),
    )
    branch = mir.Op(
        3,
        ir.Operation.BRANCH,
        "jne",
        (),
        (tested_flags,),
        kind=mir.Kind.BRANCH,
        test=mir.Kind.NE,
        target=4,
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (mask, zero_test, branch), (3, 4)),))

    result = algebraic.simplified(body, {tested_flags}, set())

    assert result.blocks[0].ops[-1].uses == (produced_flags,)
