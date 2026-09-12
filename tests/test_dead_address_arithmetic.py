import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import peephole


@pytest.mark.parametrize("flag_user", ["add", "adc", None])
def test_folded_culling_address_drops_only_unobserved_arithmetic(flag_user: str | None) -> None:
    # r_walk's shared +8 address was folded into its loads but left
    # MOV AX,SI / ADD AX,8 before another ADD and an AX overwrite.
    ax, si, di = (ir.Reg(register, 2) for register in (Register.AX, Register.SI, Register.DI))
    copy = lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (si,)), (), ())
    address = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.BINARY, "add", (ax,), (ax, ir.Imm(8, 2))), (), ())
    flags = lir.Insn(
        2, (2, 2), ir.Semantics(ir.Operation.BINARY, flag_user or "add", (di,), (di, ir.Imm(4, 2))), (), ()
    )
    overwrite = lir.Insn(3, (3, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (si,)), (), ())
    operations = (copy, address, flags, overwrite) if flag_user else (copy, address, overwrite)
    body = lir.LirBody("cull", 0, (lir.LirBlock(0, operations, ()),), {}, {})
    result = peephole.overwritten(body)
    assert (address not in result.insns) == (flag_user == "add")
    assert (copy not in result.insns) == (flag_user == "add")
    assert overwrite in result.insns
