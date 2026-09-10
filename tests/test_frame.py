"""Qrender hung in FindFrame after a spill overwrote its runtime frame link."""

import pytest
from dataclasses import replace
from iced_x86 import Register

from qbopt.backend import frame
from qbopt.model import ir, lir


@pytest.mark.parametrize("registers", [(Register.CX,), (Register.BX, Register.CX),
    (Register.AX, Register.BX, Register.CX, Register.DX, Register.SI, Register.DI)])
def test_entry_frame_size_comes_from_cx_not_call_arity(registers):
    """COM_CHECK_ARGS spilled at BP-2, corrupting FindFrame's linked list."""
    size = ir.Held(100, 2)
    init = lir.Insn(at=0, covers=(0, 3), op=None,
        what=ir.Semantics(ir.Operation.MOVE, "mov", (size,), (ir.Imm(12, 2),)),
        defines=(100,), uses=())
    operands = tuple(size if reg == Register.CX else ir.Held(reg, 2) for reg in registers)
    call = lir.Insn(at=3, covers=(3, 8), op=None,
        what=ir.Semantics(ir.Operation.CALL, "call", (), operands),
        defines=(), uses=tuple(value.value for value in operands),
        requires=tuple(zip(operands, registers)))
    body = lir.LirBody("entry", 0, (lir.LirBlock(0, (init, call), ()),), origin={}, pins={})
    owned = frame.of(body, {3: "B$ENRA"})
    assert owned.floor == -22
    assert owned.slot(200, 2) == -24
    address = replace(init, at=8, what=ir.Semantics(ir.Operation.MOVE, "lea",
        (ir.Held(101, 2),), (ir.Address(ir.Addr(ir.Space.FRAME, -32), 0),)))
    addressed = replace(body, blocks=(replace(body.blocks[0], insns=(init, call, address)),))
    assert frame.of(addressed, {3: "B$ENRA"}).slot(200, 2) == -34
    for required in ((), ((ir.Held(999, 2), Register.CX),)):
        invalid = replace(body, blocks=(replace(body.blocks[0],
            insns=(init, replace(call, requires=required))),))
        with pytest.raises(frame.Refused, match="known constant"):
            frame.of(invalid, {3: "B$ENRA"})
