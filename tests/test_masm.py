"""The jwasm printer spells each operand as select.py encodes it."""

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import masm
from qbopt.backend import lower
from qbopt.model import mir
from qbopt.objectfile.module import Addr, Space


def test_frame_address_displacement_once():
    """`lea bx,[bp-20]` for `&n` at bp-10: lower carries the displacement in
    both addr and offset, and the printer added them. asset_seek wrote n
    ten bytes below where qglsurf then read it."""
    assert masm._operand(lower.operand(mir.FrameAddress(-10, 2)), {}) == "[bp-10]"


def test_far_cell_displacement_once():
    """`es:[bx+si+4]` for field 2 of a 3-byte record: pal_install copied the
    palette's blue from the next entry's green."""
    cell = ir.Mem(Addr(Space.FAR, 2, base=Register.BX, segment=Register.ES), 1, through=Register.BX, offset=2)
    assert masm._operand(cell, {}) == "byte ptr es:[bx+2]"


def test_x87_exchange_is_fxch():
    """FloatAlloc's `fxch st(1)` printed as `xchg st(0), st(1)`, which jwasm
    refuses: 122 errors over qcport."""
    swap = ir.Semantics(ir.Operation.EXCHANGE, "fxch", (ir.St(0), ir.St(1)), (ir.St(0), ir.St(1)))
    assert masm._instruction(lir.Insn(1, (1, 1), swap, (), ()), None, {}, 0, []) == ["fxch st(1)"]


def test_arithmetic_lea_scales_its_index():
    """peephole's multiply by three has no address, only base, index and scale."""
    where = ir.Address(None, through=Register.EBX, index=Register.EBX, scale=2)
    assert masm._operand(where, {}) == "[ebx+ebx*2]"
