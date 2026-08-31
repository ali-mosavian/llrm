"""
MIR to machine bytes: the instructions this pass writes itself.

Everything emitted before this module was a rearrangement of bytes BC had
already written -- calls.py assembles from a fixed repertoire around a call
site, reencode.py re-encodes an instruction that already exists, and
lower() hands back each op's own origin span. None of them can produce an
instruction the input did not contain, which mir.lower()'s docstring names
as the missing piece and the reason a transform can delete but not rewrite.

Deliberately a small table rather than a general selector. Every entry here
exists because something measured needed it, and an op this does not know
is refused rather than approximated -- the same discipline as the rest of
the pass, and the reason a caller can treat None as "leave it alone".

Sixteen-bit code, so a 32-bit operand carries the 0x66 prefix and
`mov ecx,eax` is three bytes, not two. That matters to callers: this is the
one emitter whose output can be *longer* than what it replaces, and the
win is instructions and cycles rather than size.
"""

from iced_x86 import Code
from iced_x86 import Encoder
from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import Instruction

from qbopt.declen import BITNESS

# The 32-bit roots this can name, and their 16-bit halves.
_WIDE = {Register.EAX, Register.ECX, Register.EDX, Register.EBX, Register.ESI, Register.EDI}
_NARROW = {Register.AX, Register.CX, Register.DX, Register.BX, Register.SI, Register.DI}


def _assemble(made: Instruction, at: int) -> bytes | None:
    encoder = Encoder(BITNESS)
    try:
        encoder.encode(made, at)
    except ValueError:
        return None
    return encoder.take_buffer()


def move(into: Register_, outof: Register_, at: int = 0) -> bytes | None:
    """`mov into, outof`, or None if this cannot name that pair.

    Both registers at one width, and a width this knows. A move between
    different widths is a different instruction with different semantics --
    zero or sign extension -- and guessing which was meant is exactly what
    this refuses to do.
    """
    if into is outof:
        return b""  # a move to itself is no instruction at all
    if into in _WIDE and outof in _WIDE:
        code = Code.MOV_R32_RM32
    elif into in _NARROW and outof in _NARROW:
        code = Code.MOV_R16_RM16
    else:
        return None
    return _assemble(Instruction.create_reg_reg(code, into, outof), at)
