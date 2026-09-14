from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import spiller
from qbopt.backend.frame import Frame


def test_rematerialization_does_not_keep_the_abandoned_constant() -> None:
    # MODEL's MOD_OPEN kept mov bx,40h after rematerializing 40h at its call.
    constant = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(64, 2),)), (1,), ())
    use = lir.Insn(3, (3, 4), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)), (), (1,))
    body = lir.LirBody("rematerialized", 0, (lir.LirBlock(0, (constant, use)),), {}, {})
    result, _ = spiller.spilled(body, frozenset({1}), Frame(0))
    moves = [one for one in result.insns if one.what and one.what.op is ir.Operation.MOVE]
    assert len(moves) == 1
    assert result.insns[-1].covers == (0, 4)


def test_reordered_definition_retains_a_byte_ownership_anchor() -> None:
    prefix = lir.Insn(0, (0, 1), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Imm(0, 2),)), (), ())
    constant = lir.Insn(3, (3, 6), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(64, 2),)), (1,), ())
    moved = replace(prefix, at=1, covers=(1, 3))
    use = lir.Insn(6, (6, 7), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)), (), (1,))
    body = lir.LirBody("reordered", 0, (lir.LirBlock(0, (prefix, constant, moved, use)),), {}, {})
    result, _ = spiller.spilled(body, frozenset({1}), Frame(0))
    owner = next(one for one in result.insns if one.covers == (3, 6))
    assert owner.what == ir.Semantics(ir.Operation.NOTHING, "nop")
    assert owner.defines == owner.uses == ()


@pytest.mark.parametrize("constraint", ["requires", "delivers", "symbol"])
def test_rematerialized_definition_with_unresolved_obligations_is_kept(constraint: str) -> None:
    constant = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(64, 2),)), (1,), ())
    match constraint:
        case "requires":
            constant = replace(constant, requires=((ir.Held(1, 2), Register.AX),))
        case "delivers":
            constant = replace(constant, delivers=((ir.Held(1, 2), Register.AX),))
        case "symbol":
            constant = replace(constant, symbol=True)
    use = lir.Insn(3, (3, 4), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)), (), (1,))
    insns = (constant, use)
    body = lir.LirBody("guarded", 0, (lir.LirBlock(0, insns),), {}, {})
    result, _ = spiller.spilled(body, frozenset({1}), Frame(0))
    assert any(one.at == 0 and one.what and one.what.op is ir.Operation.MOVE for one in result.insns)
