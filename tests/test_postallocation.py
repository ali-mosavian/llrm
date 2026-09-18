"""Small Tier-1 checks for physical-register rewrites after allocation."""

from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import peephole


def _pair(operation: ir.Operation, name: str, tail: tuple[lir.Insn, ...]) -> tuple[lir.LirBody, lir.Insn]:
    left = ir.Reg(Register.EBX, 4)
    right = ir.Reg(Register.ECX, 4)
    combined = lir.Insn(
        1,
        (1, 3),
        ir.Semantics(operation, name, (left,), (left, right)),
        (10,),
        (1, 2),
    )
    copied = lir.Insn(
        3,
        (3, 3),
        ir.Semantics(ir.Operation.MOVE, "mov", (right,), (left,)),
        (11,),
        (10,),
    )
    return lir.LirBody("pair", 0, (lir.LirBlock(0, (combined, copied, *tail), ()),), {}, {}), copied


@pytest.mark.parametrize(
    "operation,name",
    [
        (ir.Operation.BINARY, "add"),
        (ir.Operation.BINARY, "and"),
        (ir.Operation.BINARY, "or"),
        (ir.Operation.BINARY, "xor"),
        (ir.Operation.MULTIPLY, "imul"),
    ],
)
def test_commutative_result_copy_uses_the_dying_other_operand(operation: ir.Operation, name: str) -> None:
    """Mandel's inner loop emitted ``imul ebx,ecx; mov ecx,ebx``.

    Both input values die at the multiply and ECX is immediately overwritten
    with its result.  The equivalent ``imul ecx,ebx`` leaves the only live
    physical result in the same place without executing the copy.
    """
    left = ir.Reg(Register.EBX, 4)
    right = ir.Reg(Register.ECX, 4)
    shift = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(ir.Operation.BINARY, "sar", (right,), (right, ir.Imm(7, 1))),
        (12,),
        (11,),
    )
    overwrite = lir.Insn(
        5,
        (5, 7),
        ir.Semantics(ir.Operation.MOVE, "mov", (left,), (ir.Imm(0, 4),)),
        (13,),
        (),
    )
    body, copied = _pair(operation, name, (shift, overwrite))

    result = peephole.transferred(body)

    assert result.insns[0].what == ir.Semantics(operation, name, (right,), (right, left))
    assert result.insns[1].what.op is ir.Operation.NOTHING
    assert result.insns[1].defines == copied.defines
    assert result.insns[2:] == (shift, overwrite)


def test_commutative_result_copy_keeps_a_still_live_first_operand() -> None:
    """Changing which multiply input is destroyed is legal only when the old destination dies."""
    left = ir.Reg(Register.EBX, 4)
    use = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (left, ir.Imm(0, 4))),
        (),
        (10,),
    )
    body, _copied = _pair(ir.Operation.MULTIPLY, "imul", (use,))

    assert peephole.transferred(body) == body


def test_commutative_result_copy_keeps_source_owned_copy_bytes() -> None:
    """A real input instruction is not the synthetic transfer this rewrite may erase."""
    left = ir.Reg(Register.EBX, 4)
    overwrite = lir.Insn(
        5,
        (5, 7),
        ir.Semantics(ir.Operation.MOVE, "mov", (left,), (ir.Imm(0, 4),)),
        (13,),
        (),
    )
    body, copied = _pair(ir.Operation.MULTIPLY, "imul", (overwrite,))
    owned = replace(copied, covers=(3, 5))
    body = replace(body, blocks=(replace(body.blocks[0], insns=(body.insns[0], owned, overwrite)),))

    assert peephole.transferred(body) == body
