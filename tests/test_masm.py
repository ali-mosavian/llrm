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


def _printed(sources: tuple, reserve: int) -> list[str]:
    move = lir.Insn(0, (0, 1), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), sources), (), ())
    leave = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.RETURN, "retf"), (), ())
    body = lir.LirBody("get", 1, (lir.LirBlock(1, (move, leave)),), {}, {})
    procedure = masm.Procedure("_get", True, True, body, reserve, {})
    return [line.strip() for line in masm._procedure(procedure, {}, 0)]


def test_frame_only_where_something_uses_it():
    """Every procedure got `push bp; mov bp,sp` .. `mov sp,bp; pop bp`: snd_mix_loops
    read one global in seven instructions where bcc used two."""
    assert _printed((ir.Reg(Register.BX, 2),), 0) == ["_get proc far", "L0_1:", "mov ax, bx", "retf", "_get endp"]
    through = ir.Mem(Addr(Space.FRAME, 6), 2, through=Register.BP)
    params = _printed((through,), 0)
    assert params[1:3] == ["push bp", "mov bp, sp"] and params[-3:-1] == ["pop bp", "retf"]
    assert "mov sp, bp" not in params and "leave" not in params


def test_reserved_frame_leaves_in_one_instruction():
    """`mov sp,bp; pop bp` where bcc writes `leave`: 261 instructions over qcport."""
    through = ir.Mem(Addr(Space.FRAME, 6), 2, through=Register.BP)
    lines = _printed((through,), 4)
    assert lines[-3:-1] == ["leave", "retf"] and "mov sp, bp" not in lines and "pop bp" not in lines


def test_callee_saves_only_what_the_convention_keeps():
    """SI and DI were pushed and popped whole: an operand-size prefix on every save
    and restore, for upper halves no Borland caller keeps across a call."""
    lines = _printed((ir.Reg(Register.ESI, 4),), 0)
    assert "push si" in lines and "pop si" in lines
    assert "push esi" not in lines and "pop esi" not in lines


def test_arithmetic_lea_scales_its_index():
    """peephole's multiply by three has no address, only base, index and scale."""
    where = ir.Address(None, through=Register.EBX, index=Register.EBX, scale=2)
    assert masm._operand(where, {}) == "[ebx+ebx*2]"
