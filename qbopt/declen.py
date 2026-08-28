#!/usr/bin/env python3
"""
Instruction length decoder for the subset BC emits, and the generator for the
256-byte table the assembly version indexes.

The point of it is that qbeWiden walks code one byte at a time and retries its
pattern at every offset, which is why it matches nothing -- and why, if it ever
did match, it would be rewriting the middle of an instruction. Knowing where
instructions begin is the whole difference.

Unknown opcodes are not guessed at. They set BAD, the caller abandons the
block, and the code is left alone. That is what keeps the table small: it only
has to cover what BC actually emits, and everything else costs a missed
optimisation rather than a corrupted program.
"""

from enum import IntFlag
from dataclasses import dataclass
from collections.abc import Iterable

M = 0x01  # has a modrm byte
I8 = 0x02  # imm8
IZ = 0x04  # imm16 or imm32, whichever the operand size says
I16 = 0x08  # imm16 regardless
PFX = 0x10  # prefix; keep going
# Transfers control, which is NOT the same as ending a basic block: 9A, E8 and
# CD all carry this and all come back. Nothing reads it; block termination needs
# the ModRM reg field to tell FF /4 and /5 from FF /2 and /3, so it belongs to
# whatever builds the blocks, not to a table bit.
XFER = 0x20
MOFF = 0x40  # moffs: a displacement sized by the address prefix, no modrm
BAD = 0x80  # not known -- give up on this block


def one_byte_opcodes() -> tuple[int, ...]:
    """Flags per opcode: what follows it, and whether it is known at all."""
    table = [BAD] * 256

    def put(codes: Iterable[int], flags: int) -> None:
        for code in codes:
            table[code] = flags

    # the eight alu operations, each in its six classic encodings
    for base in (0x00, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38):
        put(range(base, base + 4), M)  # r/m,r and r,r/m in both widths
        put([base + 4], I8)  # al, imm8
        put([base + 5], IZ)  # eAX, imm16/32
    put([0x80], M | I8)
    put([0x81], M | IZ)
    put([0x83], M | I8)
    put([0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8E, 0x8D], M)  # test/xchg/mov/lea
    put([0x8F], M)  # pop r/m
    put(range(0x40, 0x60), 0)  # inc/dec/push/pop reg
    put([0x60, 0x61, 0x98, 0x99, 0x9C, 0x9D, 0x9E, 0x9F], 0)
    put(range(0x90, 0x98), 0)  # nop, and xchg ax,r16 --
    # BC does not emit these but
    # B$DVI4 does, and the module
    # header decodes as one
    put([0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD], 0)  # clc/stc/cli/sti/cld/std
    put([0x6A], I8)
    put([0x68], IZ)  # push imm
    put([0x69], M | IZ)
    put([0x6B], M | I8)  # imul r,r/m,imm
    put([0xA0, 0xA1, 0xA2, 0xA3], MOFF)  # mov al/eAX <-> moffs
    put([0xA8], I8)
    put([0xA9], IZ)  # test al/eAX, imm
    put(range(0xB0, 0xB8), I8)  # mov r8, imm8
    put(range(0xB8, 0xC0), IZ)  # mov r32, imm
    put([0xC6], M | I8)
    put([0xC7], M | IZ)  # mov r/m, imm
    put([0xC0, 0xC1], M | I8)  # shift r/m, imm8
    put([0xD0, 0xD1, 0xD2, 0xD3], M)  # shift by 1 / by cl
    put([0xF6], M | I8)
    put([0xF7], M | IZ)  # the F6/F7 group -- see note below
    put([0xFE, 0xFF], M)  # inc/dec/call/jmp/push r/m
    put([0x26, 0x2E, 0x36, 0x3E, 0x64, 0x65, 0x66, 0x67, 0xF0, 0xF2, 0xF3], PFX)
    put([0xA4, 0xA5, 0xA6, 0xA7, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF], 0)  # string ops
    put(range(0x70, 0x80), I8 | XFER)  # jcc rel8
    put([0xEB], I8 | XFER)
    put([0xE9], IZ | XFER)
    put([0xE8], IZ | XFER)  # call rel16
    put([0xE0, 0xE1, 0xE2, 0xE3], I8 | XFER)  # loop/jcxz
    put([0xC3, 0xCB], XFER)
    put([0xC2, 0xCA], I16 | XFER)
    put([0x9A, 0xEA], XFER | 0x100 & 0xFF)  # far ptr -- handled specially below
    table[0x9A] = table[0xEA] = XFER | I16 | IZ  # seg:off = imm16 after an imm16/32
    put([0xCD], I8 | XFER)
    put([0xCF], XFER)  # int / iret
    put([0x06, 0x07, 0x0E, 0x16, 0x17, 0x1E, 0x1F], 0)  # push/pop seg
    put([0xC8], 0)  # enter -- imm16+imm8, below
    put([0xC9], 0)  # leave
    put([0xEC, 0xED, 0xEE, 0xEF], 0)  # in/out via dx
    put([0xE4, 0xE5, 0xE6, 0xE7], I8)  # in/out imm8
    put([0x62, 0x63], M)  # bound / arpl
    put([0xC4, 0xC5], M)  # les / lds
    put([0x82], M | I8)  # undocumented alias of 80
    put([0xCC], XFER)
    put([0xCE], XFER)  # int3 / into
    put([0xD7], 0)
    put([0xD4, 0xD5], I8)  # xlat / aam / aad
    put([0x9B], 0)  # wait
    put(range(0xD8, 0xE0), M)  # x87 -- modrm, no immediate
    table[0xC8] = 0x100  # marked; enter is imm16+imm8
    table[0x0F] = 0x200  # marked; two-byte escape

    # The 0F map. BC only reaches it under /G3, but the runtime and the FP
    # emulator use it freely, so a decoder that stops here stops everywhere.
    return tuple(table)


def two_byte_opcodes() -> tuple[int, ...]:
    """The same, for the 0F escape map."""
    table = [BAD] * 256

    def put(codes: Iterable[int], flags: int) -> None:
        for code in codes:
            table[code] = flags

    put(range(0x80, 0x90), IZ | XFER)  # jcc rel16/32
    put(range(0x90, 0xA0), M)  # setcc r/m8
    put(range(0x40, 0x50), M)  # cmovcc
    put([0xA0, 0xA1, 0xA8, 0xA9, 0xA2], 0)  # push/pop fs,gs / cpuid
    put([0xA3, 0xAB, 0xB3, 0xBB, 0xBC, 0xBD, 0xAF], M)  # bt* / bsf / bsr / imul
    put([0xBA], M | I8)  # bt group, imm8
    put([0xA4, 0xAC], M | I8)
    put([0xA5, 0xAD], M)  # shld / shrd
    put([0xB6, 0xB7, 0xBE, 0xBF], M)  # movzx / movsx
    put([0x00, 0x01, 0x02, 0x03], M)  # lldt/lgdt/lar/lsl group
    put([0xB2, 0xB4, 0xB5], M)  # lss / lfs / lgs
    put(range(0x20, 0x25), M)  # mov cr/dr
    put(range(0xC8, 0xD0), 0)  # bswap
    put([0x05, 0x06, 0x07, 0x08, 0x09, 0x0B, 0x30, 0x31, 0x32, 0xA6, 0xA7, 0xAA], 0)
    return tuple(table)


# F7 /0 carries an immediate but F7 /2..7 (not, neg, mul, imul, div, idiv) do
# not, so the flag above is a lie for those and the decoder corrects it by
# looking at the modrm reg field. Same for F6.
OPCODES = one_byte_opcodes()
OPCODES_0F = two_byte_opcodes()

GROUP_F = {0xF6: I8, 0xF7: IZ}


class Prefix(IntFlag):
    NONE = 0
    OPSIZE = 0x01  # 66
    ADDRSIZE = 0x02  # 67
    SEGMENT = 0x04
    REP = 0x08
    LOCK = 0x10


PREFIX_BIT = {0x66: Prefix.OPSIZE, 0x67: Prefix.ADDRSIZE, 0xF0: Prefix.LOCK, 0xF2: Prefix.REP, 0xF3: Prefix.REP}


@dataclass(frozen=True, slots=True)
class Insn:
    at: int
    length: int
    opcode: int  # 0x0F00 | second byte, for the two-byte map
    prefixes: Prefix
    opsize: int
    addrsize: int
    modrm: int | None = None
    # disp_at and imm_at are why this is a record rather than a byte count: they
    # are the offsets a fixup patches, and so the key into the module's operands.
    disp: int | None = None
    disp_at: int | None = None
    disp_len: int = 0
    imm_at: int | None = None
    imm_len: int = 0

    @property
    def end(self) -> int:
        return self.at + self.length

    @property
    def mod(self) -> int:
        return self.modrm >> 6 if self.modrm is not None else 0

    @property
    def reg(self) -> int:
        return (self.modrm >> 3) & 7 if self.modrm is not None else 0

    @property
    def rm(self) -> int:
        return self.modrm & 7 if self.modrm is not None else 0


def _modrm_bytes(code: bytes, at: int, mod: int, rm: int, addrsize: int) -> tuple[int, int] | None:
    """(where the displacement starts, how many bytes) after the ModRM byte."""
    if addrsize == 16:
        match (mod, rm):
            case (0, 6) | (2, _):
                return at, 2
            case (1, _):
                return at, 1
            case _:
                return at, 0
    if mod == 3:  # a register, so no sib and no displacement
        return at, 0
    if rm == 4:  # a sib byte, whose base can itself demand a disp32
        if at >= len(code):
            return None
        sib, at = code[at], at + 1
        if mod == 0 and sib & 7 == 5:
            return at, 4
    match (mod, rm):
        case (0, 5) | (2, _):
            return at, 4
        case (1, _):
            return at, 1
        case _:
            return at, 0


def to_signed(raw: int, width: int) -> int:
    """A `width`-byte field read as two's complement."""
    bits = width * 8
    return raw - (1 << bits) if raw >= 1 << (bits - 1) else raw


def decode(code: bytes, at: int, opsize: int = 16, addrsize: int = 16) -> Insn | None:
    """The instruction at `at`, or None if the opcode is unknown."""
    start = at
    prefixes = Prefix.NONE
    while at < len(code) and OPCODES[code[at]] & PFX:
        if OPCODES[code[at]] == BAD:
            return None
        prefixes |= PREFIX_BIT.get(code[at], Prefix.SEGMENT)
        if code[at] == 0x66:
            opsize = 48 - opsize  # 16<->32
        elif code[at] == 0x67:
            addrsize = 48 - addrsize
        at += 1
    if at >= len(code) or OPCODES[code[at]] == BAD:
        return None

    opcode, flags = code[at], OPCODES[code[at]]
    at += 1

    if flags == 0x200:  # 0F: take the second map
        if at >= len(code):
            return None
        opcode, flags = 0x0F00 | code[at], OPCODES_0F[code[at]]
        at += 1
        if flags == BAD:
            return None
    elif flags == 0x100:  # enter imm16, imm8
        if at + 3 > len(code):
            return None
        return Insn(start, at + 3 - start, opcode, prefixes, opsize, addrsize, imm_at=at, imm_len=3)

    immediate = flags
    if opcode in GROUP_F:
        if at >= len(code):
            return None
        immediate = M | GROUP_F[opcode] if (code[at] >> 3) & 7 in (0, 1) else M

    modrm = disp_at = None
    disp_len = 0
    if flags & M:
        if at >= len(code):
            return None
        modrm, at = code[at], at + 1
        placed = _modrm_bytes(code, at, modrm >> 6, modrm & 7, addrsize)
        if placed is None:
            return None
        disp_at, disp_len = placed
        at = disp_at + disp_len
    if flags & MOFF:  # a displacement with no ModRM in front of it
        disp_at, disp_len = at, 2 if addrsize == 16 else 4
        at += disp_len

    imm_at, imm_len = None, 0
    if immediate & I8:
        imm_len += 1
    if immediate & IZ:
        imm_len += 2 if opsize == 16 else 4
    if immediate & I16:
        imm_len += 2
    if imm_len:
        imm_at, at = at, at + imm_len

    if at > len(code):
        return None
    disp = None
    if disp_at is not None and disp_len:
        disp = int.from_bytes(code[disp_at : disp_at + disp_len], "little")
    return Insn(start, at - start, opcode, prefixes, opsize, addrsize, modrm, disp, disp_at, disp_len, imm_at, imm_len)


def length(code: bytes, at: int, opsize: int = 16, addrsize: int = 16) -> int | None:
    """Bytes of the instruction at `at`, or None if the opcode is unknown."""
    found = decode(code, at, opsize, addrsize)
    return found.length if found else None


def run(code: bytes, start: int, end: int) -> tuple[list[Insn], int | None]:
    """Every instruction from `start`, and the offset where decoding gave up."""
    found = []
    at = start
    while at < end:
        insn = decode(code, at)
        if insn is None:
            return found, at
        found.append(insn)
        at = insn.end
    return found, None


if __name__ == "__main__":
    print("table:", sum(1 for x in OPCODES if x != BAD), "of 256 opcodes known")
