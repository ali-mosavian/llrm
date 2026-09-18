"""Fast regressions for exact-loop expansion budgets."""

from qbopt.model import ir
from qbopt.model import mir
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
