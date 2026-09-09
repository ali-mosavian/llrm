import pytest

from qbopt import ir
from qbopt import mir
from qbopt import ssa


@pytest.mark.parametrize("reader", ["none", "argument", "memory", "root"])
def test_unused_phi_cycles_are_pruned_but_real_dependencies_survive(reader: str) -> None:
    seed, first, second = mir.Value(1, 0), mir.Value(2, 1), mir.Value(3, 1)
    phis = (mir.Phi(first, {0: seed, 1: second}), mir.Phi(second, {0: seed, 1: first}))
    ops = ()
    if reader == "argument":
        ops = (mir.Op(2, ir.Operation.PUSH, "push", (), (first,), kind=mir.Kind.ARG, args=(mir.Held(first, 2),)),)
    if reader == "memory":
        ref = mir.MemRef(None, 2, base=first)
        ops = (mir.Op(2, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.LOAD, loads=(ref,), args=(mir.Cell(ref),)),)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (), (1,)), mir.MirBlock(1, phis, ops, (1,))))
    result = ssa.pruned_phis(body, {first} if reader == "root" else set())
    assert result.blocks[1].phis == (() if reader == "none" else phis)
