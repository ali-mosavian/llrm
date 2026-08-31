"""
qbopt/select.py: the first instructions this pass writes rather than moves.

Everything before it rearranged bytes BC had already emitted. So the tests
are about refusal as much as output -- an emitter that guesses at a form it
does not know produces a working program that computes something else.
"""

from iced_x86 import Code
from iced_x86 import Decoder
from iced_x86 import Register

from qbopt import select
from qbopt.declen import BITNESS

ROOTS = (Register.EAX, Register.ECX, Register.EDX, Register.EBX, Register.ESI, Register.EDI)
HALVES = (Register.AX, Register.CX, Register.DX, Register.BX, Register.SI, Register.DI)


def test_a_move_decodes_back_to_the_move_that_was_asked_for() -> None:
    """Round-tripped through the decoder rather than compared to a literal:
    a hand-written expectation can be wrong in the same direction twice."""
    for into in ROOTS:
        for outof in ROOTS:
            if into is outof:
                continue
            code = select.move(into, outof)
            assert code is not None
            decoded = next(iter(Decoder(BITNESS, code, ip=0)))
            assert decoded.code == Code.MOV_R32_RM32
            assert decoded.op0_register == into
            assert decoded.op1_register == outof


def test_a_sixteen_bit_move_is_shorter_than_a_thirty_two_bit_one() -> None:
    """This is 16-bit code, so a 32-bit operand carries the 0x66 prefix.

    Worth a test because it is the fact that makes this emitter's output
    able to be LONGER than what it replaces -- three bytes against the two
    pushes it stands in for -- and a caller that assumed otherwise would
    silently overrun.
    """
    wide = select.move(Register.ECX, Register.EAX)
    narrow = select.move(Register.CX, Register.AX)
    assert wide is not None and narrow is not None
    assert len(wide) == 3 and len(narrow) == 2
    assert wide[0] == 0x66


def test_a_move_to_itself_is_no_instruction() -> None:
    assert select.move(Register.EAX, Register.EAX) == b""


def test_mixed_widths_are_refused_rather_than_guessed() -> None:
    """`mov ecx,ax` is not a move -- it is a zero or sign extension, and
    which one was meant is not something this can know."""
    assert select.move(Register.ECX, Register.AX) is None
    assert select.move(Register.AX, Register.ECX) is None


def test_registers_this_does_not_name_are_refused() -> None:
    """bp and the segment registers are where values live, not values."""
    assert select.move(Register.EBP, Register.EAX) is None
    assert select.move(Register.EAX, Register.ES) is None
    assert select.move(Register.ESP, Register.EAX) is None
