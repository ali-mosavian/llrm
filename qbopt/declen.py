"""
Instructions, decoded by iced-x86.

This was a hand-built table of the subset BC emits. It was right about what it
covered and silent about everything else, and three of the things it did not
cover turned out to matter: what a shift leaves undefined, which transfers of
control come back, and where an immediate begins. iced answers all of them from
a complete model of the architecture.

What is kept is the shape the rest of the pass reads: an Insn is where an
instruction sits and where its displacement and immediate fields are, because
those are the offsets a fixup patches and so the key into a module's operands.
The instruction itself comes along, so nothing here has to mirror iced's model
or keep up with it.
"""

from dataclasses import dataclass

from iced_x86 import OpKind
from iced_x86 import Decoder
from iced_x86 import Register
from iced_x86 import Instruction

# BC targets an 8086; everything qbopt emits is a 386 form in a 16-bit segment.
BITNESS = 16

MEMORY = OpKind.MEMORY
NO_REGISTER = Register.NONE


@dataclass(frozen=True, slots=True)
class Insn:
    at: int
    length: int
    insn: Instruction
    disp_at: int | None = None
    disp_len: int = 0
    imm_at: int | None = None
    imm_len: int = 0

    @property
    def end(self) -> int:
        return self.at + self.length

    @property
    def code(self) -> int:
        return self.insn.code

    @property
    def flow(self) -> int:
        return self.insn.flow_control

    @property
    def reads(self) -> int:
        return self.insn.rflags_read

    @property
    def writes(self) -> int:
        """Every flag this leaves other than as it found it.

        iced separates the four ways that happens: computed, forced to zero,
        forced to one, and left undefined. All four destroy what was there, and
        `and` is the one that shows it -- it computes four flags and clears two,
        so reading only the computed ones says it leaves CF alone. It does not.
        """
        return self.insn.rflags_written | self.insn.rflags_cleared | self.insn.rflags_set | self.insn.rflags_undefined

    @property
    def target(self) -> int | None:
        """Where a self-relative branch goes, or None if it is not one."""
        if self.insn.op0_kind in (OpKind.NEAR_BRANCH16, OpKind.NEAR_BRANCH32):
            return self.insn.near_branch_target
        return None

    @property
    def memory_base(self) -> int:
        return self.insn.memory_base

    @property
    def displacement(self) -> int:
        """The displacement as the instruction means it, sign and all.

        iced gives it already widened to the address size, so the sign comes
        from there and not from however many bytes it was encoded in.
        """
        return to_signed(self.insn.memory_displacement, BITNESS // 8) if self.disp_len else 0

    def reads_memory(self, operand: int) -> bool:
        return (self.insn.op0_kind if operand == 0 else self.insn.op1_kind) == MEMORY

    def register(self, operand: int) -> int:
        return self.insn.op0_register if operand == 0 else self.insn.op1_register


def to_signed(raw: int, width: int) -> int:
    """A `width`-byte field read as two's complement."""
    bits = width * 8
    return raw - (1 << bits) if raw >= 1 << (bits - 1) else raw


def decode(code: bytes, at: int) -> Insn | None:
    """The instruction at `at`, or None if the bytes are not one."""
    if at >= len(code):
        return None
    decoder = Decoder(BITNESS, code[at:], ip=at)
    if not decoder.can_decode:
        return None
    insn = decoder.decode()
    if insn.is_invalid or at + insn.len > len(code):
        return None

    where = decoder.get_constant_offsets(insn)
    return Insn(
        at=at,
        length=insn.len,
        insn=insn,
        disp_at=at + where.displacement_offset if where.has_displacement else None,
        disp_len=where.displacement_size if where.has_displacement else 0,
        imm_at=at + where.immediate_offset if where.has_immediate else None,
        imm_len=where.immediate_size if where.has_immediate else 0,
    )


def length(code: bytes, at: int) -> int | None:
    found = decode(code, at)
    return found.length if found else None


def run(code: bytes, start: int, end: int) -> tuple[list[Insn], int | None]:
    """Every instruction from `start`, and the offset where decoding gave up."""
    found = []
    at = start
    while at < end:
        insn = decode(code, at)
        if insn is None or insn.end > end:
            return found, at
        found.append(insn)
        at = insn.end
    return found, None
