"""A far cell reached as `[base+index*scale]`, and the zero extension that
puts a word in the 32-bit register such an address is built from."""

from iced_x86 import Decoder
from iced_x86 import Register
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

from qbopt.model import ir
from qbopt.backend import select
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def _text(code: bytes) -> str:
    decoder = Decoder(16, code)
    formatter = Formatter(FormatterSyntax.INTEL)
    return "; ".join(formatter.format(one) for one in decoder)


def test_a_far_cell_is_encoded_with_its_scaled_index() -> None:
    cell = ir.Mem(
        Addr(Space.FAR, 0, segment=Register.ES),
        2,
        through=Register.ESI,
        base=ir.Held(1, 4),
        index=ir.Held(2, 4),
        scale=2,
        index_through=Register.ECX,
    )
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (cell,))
    emitted = select.emit(what, 0)
    assert emitted is not None
    assert emitted.code[:2] == b"\x26\x67"
    assert _text(emitted.code) == "mov ax,es:[esi+ecx*2]"


def test_a_word_is_zero_extended_into_a_dword_register() -> None:
    what = ir.Semantics(ir.Operation.EXTEND, "movzx", (ir.Reg(Register.ECX, 4),), (ir.Reg(Register.CX, 2),))
    emitted = select.emit(what, 0)
    assert emitted is not None
    assert _text(emitted.code) == "movzx ecx,cx"


def test_a_word_index_is_encoded_with_word_addressing() -> None:
    """`[bx+si]` wraps as the word add it replaces; 32-bit addressing would not."""
    cell = ir.Mem(
        Addr(Space.FAR, 0, segment=Register.FS),
        1,
        through=Register.BX,
        base=ir.Held(1, 2),
        index=ir.Held(2, 2),
        index_through=Register.SI,
    )
    what = ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (ir.Reg(Register.CL, 1),))
    emitted = select.emit(what, 0)
    assert emitted is not None
    assert _text(emitted.code) == "mov fs:[bx+si],cl"
