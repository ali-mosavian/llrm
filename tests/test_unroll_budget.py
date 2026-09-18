"""Fast regressions for exact-loop expansion budgets."""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.optimize import profit
from qbopt.optimize import unroll
from qbopt.model.passes import Where
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


def test_large_complete_peel_must_erase_its_growth_to_cross_the_profile_budget() -> None:
    """Shellsort encoded 64 initializer stores for a 3.6% estimated saving.

    A target may bound ordinary full expansion while still permitting a long
    exact loop that constant-folds to a body smaller than the original loop.
    Short profitable loops remain governed by the ordinary cost calculation.
    """
    costs = OperationCosts(add=1, branch=50, move=1)
    where = Where(costs=costs, max_unroll_iterations=16)
    original = _loop()

    assert unroll._rejection(original, _straight(3), 1, 64, where) == "iteration-growth"
    assert unroll._rejection(original, _straight(1), 1, 64, where) is None
    assert unroll._rejection(original, _straight(3), 1, 16, where) is None


def test_bounded_complete_peel_amortizes_growth_over_its_exact_trip_count() -> None:
    """CRC retained its nine-trip outer loop and reloaded every constant byte.

    The specialized straight-line body saves 38% of the target-priced dynamic
    work, but charging every clone as though code growth executed once per
    iteration rejected it.  A proven bounded trip count amortizes that static
    cost; the profile iteration ceiling remains the independent size guard.
    """
    costs = OperationCosts(add=2, branch=7, move=2)
    where = Where(costs=costs, registers=6, max_unroll_iterations=16)

    assert unroll._rejection(_loop(), _straight(25), 1, 9, where) is None


def test_bounded_peel_with_spill_risk_pays_its_complete_growth() -> None:
    """Matmul grew from 421 to 847 instructions and from 4,070 to 4,812
    executed 386 cost units after its eight-row loop was completely peeled.

    Amortizing static growth is safe for CRC's register-fitting scalar chain,
    but not for a candidate already known to exceed the target's capacity:
    MIR's spill estimate is a lower bound, and expansion magnifies any gap
    between that bound and the constrained allocation.  Such a candidate must
    erase its expansion before it can replace the loop.
    """
    costs = OperationCosts(add=1, branch=5, move=2, load=1, store=1)
    where = Where(costs=costs, registers=3, max_unroll_iterations=16)

    assert profit.spill_risk(_pressured(), costs, where.registers) > 0
    assert unroll._rejection(_loop(), _pressured(), 1, 2, where) == "growth"


def test_spill_prone_complete_peel_respects_the_sequence_budget() -> None:
    """P5 matmul crossed from 187 to 459 MIR operations, then selected 958
    instructions where the bounded form selected 421.

    GCC independently caps a completely peeled sequence at 200 estimated
    instructions.  Apply the same machine-neutral profile budget when MIR
    already predicts spills; register-fitting constant specialization remains
    governed by its separate profitability calculation.
    """
    costs = OperationCosts(add=1, branch=100, move=1, load=1, store=1)
    where = Where(
        costs=costs,
        registers=3,
        max_unroll_iterations=16,
        max_unrolled_operations=200,
    )

    assert profit.spill_risk(_large_pressured(), costs, where.registers) > 0
    assert unroll._rejection(_loop(), _large_pressured(), 1, 8, where) == "operation-growth"


def test_structural_saving_must_pay_for_unavoidable_pressure() -> None:
    """Matmul saved 60 final instructions but raised 386 cost by 1,407.

    Its scalarized clone left more non-rematerializable values live than the
    target could hold, and semantic profitability treated their eventual
    stores and reloads as free.  Charge only the cheapest spill set required
    at a pressure peak; even that lower bound is enough to refuse this shape.
    """
    costs = OperationCosts(add=1, branch=2, move=1, load=10, store=10)
    where = Where(costs=costs, registers=2)

    assert unroll._rejection(_loop(), _pressured(), 1, 8, where) == "pressure"


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
