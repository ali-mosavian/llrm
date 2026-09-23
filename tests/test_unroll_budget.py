"""Fast regressions for exact-loop expansion budgets."""

from dataclasses import replace


from qbopt.model import ir
from qbopt.model import mir
from qbopt.optimize import profit
from qbopt.model.passes import OperationCosts


def _loop() -> mir.MirBody:
    add = mir.Op(1, ir.Operation.BINARY, "add", (), (), kind=mir.Kind.ADD)
    branch = mir.Op(1, ir.Operation.BRANCH, "jne", (), (), kind=mir.Kind.BRANCH, target=1)
    return mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (), (1,)),
            mir.MirBlock(1, (), (add, branch), (1, 2)),
            mir.MirBlock(2, (), (), ()),
        ),
    )


def _straight(count: int) -> mir.MirBody:
    moves = tuple(mir.Op(at, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.COPY) for at in range(count))
    return mir.MirBody(0, (mir.MirBlock(0, (), moves, (2,)), mir.MirBlock(2, (), (), ())))


def _pressured() -> mir.MirBody:
    """Five cheap operations whose four results overlap at the final use."""
    sources = tuple(mir.Value(number, 0) for number in range(1, 5))
    results = tuple(mir.Value(number + 4, number) for number in range(1, 5))
    definitions = tuple(
        mir.Op(
            number,
            ir.Operation.BINARY,
            "add",
            (result,),
            (source,),
            kind=mir.Kind.ADD,
            args=(mir.Held(source, 2), mir.Const(number, 2)),
            results=(mir.Held(result, 2),),
        )
        for number, source, result in zip(range(1, 5), sources, results, strict=True)
    )
    total = mir.Value(9, 5)
    consume = mir.Op(
        5,
        ir.Operation.BINARY,
        "add",
        (total,),
        results,
        kind=mir.Kind.ADD,
        args=tuple(mir.Held(value, 2) for value in results),
        results=(mir.Held(total, 2),),
    )
    return mir.MirBody(0, (mir.MirBlock(0, (), (*definitions, consume), (2,)), mir.MirBlock(2, (), (), ())))


def _pressure_waves() -> mir.MirBody:
    """Two independent groups which each exceed a two-register capacity."""
    ops = []
    fresh = 1
    for wave in range(2):
        values = tuple(mir.Value(fresh + index, wave * 10 + index) for index in range(3))
        fresh += len(values)
        ops.extend(
            mir.Op(
                value.at,
                ir.Operation.BINARY,
                "add",
                (value,),
                (),
                kind=mir.Kind.ADD,
                args=(mir.Const(index, 2), mir.Const(wave, 2)),
                results=(mir.Held(value, 2),),
            )
            for index, value in enumerate(values)
        )
        ops.append(
            mir.Op(
                wave * 10 + 4,
                ir.Operation.BINARY,
                "add",
                (),
                values,
                kind=mir.Kind.ADD,
                args=tuple(mir.Held(value, 2) for value in values),
            )
        )
    return mir.MirBody(0, (mir.MirBlock(0, (), tuple(ops), ()),))


def _large_pressured() -> mir.MirBody:
    """A spill-prone expanded sequence just above GCC's default ceiling."""
    body = _pressured()
    padding = tuple(mir.Op(at, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.COPY) for at in range(6, 202))
    block = replace(body.blocks[0], ops=(*body.blocks[0].ops, *padding))
    return replace(body, blocks=(block,))


def _floating_pressure() -> mir.MirBody:
    """Three x87 values overlap but consume no integer-register capacity."""
    values = tuple(mir.Value(index, index) for index in range(1, 4))
    definitions = tuple(
        mir.Op(
            value.at,
            ir.Operation.FLOAT_LOAD,
            "fld",
            (value,),
            (),
            kind=mir.Kind.FLOAD,
            results=(mir.Held(value, 10),),
        )
        for value in values
    )
    consume = mir.Op(
        4,
        ir.Operation.BINARY,
        "fadd",
        (),
        values,
        kind=mir.Kind.FADD,
        args=tuple(mir.Held(value, 10) for value in values),
    )
    return mir.MirBody(0, (mir.MirBlock(0, (), (*definitions, consume), ()),))


def test_pressure_prices_independent_spill_waves() -> None:
    """Matmul's scalar leaves exceeded capacity in successive, disjoint groups.

    Taking only the largest pressure peak priced one spill even though no one
    spilled value could relieve both groups.  Each independent wave requires
    its own store/reload pair.
    """
    costs = OperationCosts(load=10, store=10)

    assert profit.spill_risk(_pressure_waves(), costs, 2) == 40


def test_integer_pressure_does_not_consume_x87_values() -> None:
    """The target's six-value capacity describes GPRs, not the x87 stack."""
    costs = OperationCosts(load=10, store=10)

    assert profit.spill_risk(_floating_pressure(), costs, 1) == 0
