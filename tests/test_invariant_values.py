"""Fast LICM regressions for complete values that initialize nested loops."""

from qbopt.model import ir
from qbopt.model import mir
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
