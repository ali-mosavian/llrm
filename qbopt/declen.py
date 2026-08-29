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

from enum import IntEnum
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


# Under /FPi, BC does not emit x87 instructions. It emits the emulator's
# interrupts, and Open Watcom's bld/watcom/h/fppatche.h names the whole
# protocol by the library symbol each one is patched through:
#
#   int 34h..3Bh   FIDRQQ    the ESC opcodes D8..DF, operand inline
#   int 3Ch        FIxRQQ    a segment override; the real ESC opcode follows
#   int 3Dh        FIWRQQ    the WAIT instruction, and nothing follows
#
# So `CD 35 46 C8` is `D9 46 C8`, fld dword [bp-38h] -- two bytes standing in
# for one, with the operand inline, and a walk that reads the int as an
# ordinary interrupt lands in the middle of that operand. The other two stand
# in for a whole instruction or a bare prefix and have no operand of their own,
# which is why they happen to decode at the right length even when read as
# interrupts. They are still decoded here, because a prefix separated from its
# opcode is one edit away from being split.
#
# There are 1422 of the first, 201 of the second and 507 of the third in
# qb-qrender, and the first kind is why reachability explained two of its
# fifteen modules before this. Every one of that program's int 3Ch sites is
# followed by a byte in D8..DF, which is the shape asserted below.
EMULATED = range(0x34, 0x3C)


class Stands(IntEnum):
    SEGMENTED = 0x3C  # for a segment override, with the real ESC opcode after it
    FWAIT = 0x3D  # for the whole of WAIT, with nothing after it


WAIT = 0x9B
ESC = range(0xD8, 0xE0)
INTERRUPT = 0xCD
STANDS_IN = range(EMULATED.start, Stands.FWAIT + 1)


def stood_in_for(code: bytes, at: int) -> tuple[bytes, int] | None:
    """The bytes the emulated site means, and how many of them the int hides.

    The site is always two bytes wide. What it replaces is not: an ESC opcode,
    a one-byte segment override, or a whole WAIT -- so the count says how much
    of the decode came out of the interrupt rather than from after it.
    """
    tail = code[at + 2 : at + 2 + 15]
    match code[at + 1]:
        case escape if escape in EMULATED:
            return bytes([ESC.start + escape - EMULATED.start]) + tail, 1
        case Stands.SEGMENTED if tail[:1] and tail[0] in ESC:
            return tail, 0
        case Stands.FWAIT:
            return bytes([WAIT]), 1
        case _:
            return None


def emulated(code: bytes, at: int) -> Insn | None:
    """An x87 instruction wearing the emulator's interrupt as its first byte."""
    found = stood_in_for(code, at)
    if found is None:
        return None
    stood_in, hidden = found
    decoder = Decoder(BITNESS, stood_in, ip=0)
    if not decoder.can_decode:
        return None
    insn = decoder.decode()
    if insn.is_invalid:
        return None
    length = 2 + insn.len - hidden
    if at + length > len(code):
        return None

    operand = at + 2 - hidden
    where = decoder.get_constant_offsets(insn)
    return Insn(
        at=at,
        length=length,
        insn=insn,
        disp_at=operand + where.displacement_offset if where.has_displacement else None,
        disp_len=where.displacement_size if where.has_displacement else 0,
        imm_at=operand + where.immediate_offset if where.has_immediate else None,
        imm_len=where.immediate_size if where.has_immediate else 0,
    )


def decode(code: bytes, at: int) -> Insn | None:
    """The instruction at `at`, or None if the bytes are not one."""
    if at >= len(code):
        return None
    if code[at] == INTERRUPT and at + 1 < len(code) and code[at + 1] in STANDS_IN:
        if (found := emulated(code, at)) is not None:
            return found
        # int 3Ch is the only one that can still be an ordinary interrupt: it
        # stands in for a prefix, so with no ESC opcode after it nothing is
        # being emulated. For the others, refusing beats reading an operand as
        # instructions.
        if code[at + 1] != Stands.SEGMENTED:
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
