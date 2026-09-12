import pytest
from iced_x86 import OpKind
from iced_x86 import Decoder
from iced_x86 import Register

from qbopt.model import ir
from qbopt.backend import select
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


@pytest.mark.parametrize(
    "source,operand",
    [
        (ir.Mem(Addr(Space.FRAME, -0x0E), 2, Register.BP, 0, 2), ("memory", Register.BP, 0xFFF2)),
        (ir.Reg(Register.BX, 2), ("register", Register.BX)),
        (ir.Reg(Register.CX, 2), ("register", Register.CX)),
    ],
)
def test_a_product_into_another_register_keeps_its_operand(source, operand):
    """qbdemo's WHITEFADE multiplies `[bp-0Eh]` by 2 into cx before its palette
    loop; the operand was dropped and `imul cx,2` doubled whatever the runtime
    call left in cx, so marks 2 and 4 dumped the wrong palette."""
    what = ir.Semantics(ir.Operation.MULTIPLY, "imul", (ir.Reg(Register.CX, 2),), (source, ir.Imm(2, 2)))
    (insn,) = Decoder(16, select.emit(what).code)
    read = (
        ("memory", insn.memory_base, insn.memory_displacement)
        if insn.op1_kind == OpKind.MEMORY
        else ("register", insn.op1_register)
    )
    assert (insn.op0_register, read, insn.immediate(2)) == (Register.CX, operand, 2)
