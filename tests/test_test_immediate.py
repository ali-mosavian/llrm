import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.backend import select
from qbopt.frontend.declen import decode
from qbopt.objectfile.module import Space


@pytest.mark.parametrize("width,value", [(1, 0x80), (2, 0x8000), (4, 0x80000000), (2, 1)])
@pytest.mark.parametrize("memory", [False, True])
def test_test_immediate_keeps_mask_and_flags(width: int, value: int, memory: bool) -> None:
    # R_WALK 0386 TEST [bp+6],8000h refused at final emission.
    operand = (
        ir.Mem(ir.Addr(Space.FRAME, 6), width, Register.BP, 0, 1)
        if memory
        else ir.Reg({1: Register.AL, 2: Register.AX, 4: Register.EAX}[width], width)
    )
    made = select.emit(ir.Semantics(ir.Operation.COMPARE, "test", (), (operand, ir.Imm(value, width))))
    assert made is not None
    decoded = decode(made.code, 0)
    assert decoded is not None
    assert str(decoded.insn).startswith("test ")
    assert decoded.insn.immediate(1) & ((1 << (width * 8)) - 1) == value
    reference = decode(bytes.fromhex("f746060080"), 0)
    assert reference is not None
    assert decoded.insn.rflags_written == reference.insn.rflags_written
    assert decoded.insn.rflags_cleared == reference.insn.rflags_cleared
    if memory and width == 2 and value == 0x8000:
        assert made.code == bytes.fromhex("f746060080")
