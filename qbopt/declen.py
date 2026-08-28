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

M    = 0x01   # has a modrm byte
I8   = 0x02   # imm8
IZ   = 0x04   # imm16 or imm32, whichever the operand size says
I16  = 0x08   # imm16 regardless
PFX  = 0x10   # prefix; keep going
FLOW = 0x20   # changes control flow, so a block ends here
MOFF = 0x40   # moffs: a displacement sized by the address prefix, no modrm
BAD  = 0x80   # not known -- give up on this block

T = [BAD] * 256

def put(codes, flags):
    for c in codes: T[c] = flags

# the eight alu operations, each in its six classic encodings
for base in (0x00, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38):
    put(range(base, base+4), M)          # r/m,r and r,r/m in both widths
    put([base+4], I8)                    # al, imm8
    put([base+5], IZ)                    # eAX, imm16/32
put([0x80], M|I8); put([0x81], M|IZ); put([0x83], M|I8)
put([0x84,0x85,0x86,0x87,0x88,0x89,0x8A,0x8B,0x8C,0x8E,0x8D], M)  # test/xchg/mov/lea
put([0x8F], M)                                                    # pop r/m
put(range(0x40,0x60), 0)                 # inc/dec/push/pop reg
put([0x60,0x61,0x98,0x99,0x9C,0x9D,0x9E,0x9F], 0)
put(range(0x90,0x98), 0)                 # nop, and xchg ax,r16 --
                                         # BC does not emit these but
                                         # B$DVI4 does, and the module
                                         # header decodes as one
put([0xF8,0xF9,0xFA,0xFB,0xFC,0xFD], 0)  # clc/stc/cli/sti/cld/std
put([0x6A], I8); put([0x68], IZ)         # push imm
put([0x69], M|IZ); put([0x6B], M|I8)     # imul r,r/m,imm
put([0xA0,0xA1,0xA2,0xA3], MOFF)         # mov al/eAX <-> moffs
put([0xA8], I8); put([0xA9], IZ)         # test al/eAX, imm
put(range(0xB0,0xB8), I8)                # mov r8, imm8
put(range(0xB8,0xC0), IZ)                # mov r32, imm
put([0xC6], M|I8); put([0xC7], M|IZ)     # mov r/m, imm
put([0xC0,0xC1], M|I8)                   # shift r/m, imm8
put([0xD0,0xD1,0xD2,0xD3], M)            # shift by 1 / by cl
put([0xF6], M|I8); put([0xF7], M|IZ)     # the F6/F7 group -- see note below
put([0xFE,0xFF], M)                      # inc/dec/call/jmp/push r/m
put([0x26,0x2E,0x36,0x3E,0x64,0x65,0x66,0x67,0xF0,0xF2,0xF3], PFX)
put([0xA4,0xA5,0xA6,0xA7,0xAA,0xAB,0xAC,0xAD,0xAE,0xAF], 0)       # string ops
put(range(0x70,0x80), I8|FLOW)           # jcc rel8
put([0xEB], I8|FLOW); put([0xE9], IZ|FLOW)
put([0xE8], IZ|FLOW)                     # call rel16
put([0xE0,0xE1,0xE2,0xE3], I8|FLOW)      # loop/jcxz
put([0xC3,0xCB], FLOW); put([0xC2,0xCA], I16|FLOW)
put([0x9A,0xEA], FLOW|0x100 & 0xFF)      # far ptr -- handled specially below
T[0x9A] = T[0xEA] = FLOW | I16 | IZ      # seg:off = imm16 after an imm16/32
put([0xCD], I8|FLOW); put([0xCF], FLOW)  # int / iret
put([0x06,0x07,0x0E,0x16,0x17,0x1E,0x1F], 0)      # push/pop seg
put([0xC8], 0)                                    # enter -- imm16+imm8, below
put([0xC9], 0)                                    # leave
put([0xEC,0xED,0xEE,0xEF], 0)                     # in/out via dx
put([0xE4,0xE5,0xE6,0xE7], I8)                    # in/out imm8
put([0x62,0x63], M)                               # bound / arpl
put([0xC4,0xC5], M)                               # les / lds
put([0x82], M|I8)                                 # undocumented alias of 80
put([0xCC], FLOW); put([0xCE], FLOW)              # int3 / into
put([0xD7], 0); put([0xD4,0xD5], I8)              # xlat / aam / aad
put([0x9B], 0)                                    # wait
put(range(0xD8,0xE0), M)                          # x87 -- modrm, no immediate
T[0xC8] = 0x100                                   # marked; enter is imm16+imm8
T[0x0F] = 0x200                                   # marked; two-byte escape

# The 0F map. BC only reaches it under /G3, but the runtime and the FP
# emulator use it freely, so a decoder that stops here stops everywhere.
T0F = [BAD] * 256
def put0F(codes, flags):
    for c in codes: T0F[c] = flags
put0F(range(0x80,0x90), IZ|FLOW)                  # jcc rel16/32
put0F(range(0x90,0xA0), M)                        # setcc r/m8
put0F(range(0x40,0x50), M)                        # cmovcc
put0F([0xA0,0xA1,0xA8,0xA9,0xA2], 0)              # push/pop fs,gs / cpuid
put0F([0xA3,0xAB,0xB3,0xBB,0xBC,0xBD,0xAF], M)    # bt* / bsf / bsr / imul
put0F([0xBA], M|I8)                               # bt group, imm8
put0F([0xA4,0xAC], M|I8); put0F([0xA5,0xAD], M)   # shld / shrd
put0F([0xB6,0xB7,0xBE,0xBF], M)                   # movzx / movsx
put0F([0x00,0x01,0x02,0x03], M)                   # lldt/lgdt/lar/lsl group
put0F([0xB2,0xB4,0xB5], M)                        # lss / lfs / lgs
put0F(range(0x20,0x25), M)                        # mov cr/dr
put0F(range(0xC8,0xD0), 0)                        # bswap
put0F([0x05,0x06,0x07,0x08,0x09,0x0B,0x30,0x31,0x32,0xA6,0xA7,0xAA], 0)

# F7 /0 carries an immediate but F7 /2..7 (not, neg, mul, imul, div, idiv) do
# not, so the flag above is a lie for those and the decoder corrects it by
# looking at the modrm reg field. Same for F6.
GROUP_F = {0xF6: I8, 0xF7: IZ}

def length(b, i, opsize=16, addrsize=16):
    """Bytes of the instruction at b[i], or None if the opcode is unknown."""
    start = i
    seg = False
    while i < len(b):
        f = T[b[i]]
        if f == BAD: return None
        if f & PFX:
            if b[i] == 0x66: opsize = 48 - opsize      # 16<->32
            elif b[i] == 0x67: addrsize = 48 - addrsize
            i += 1
            continue
        break
    else:
        return None
    op = b[i]; f = T[op]; i += 1

    if f == 0x200:                                 # 0F: take the second map
        if i >= len(b): return None
        op = b[i]; f = T0F[op]; i += 1
        if f == BAD: return None
    elif f == 0x100:                               # enter imm16, imm8
        return i + 3 - start

    imm = f
    if op in GROUP_F:
        if i >= len(b): return None
        if (b[i] >> 3) & 7 in (0, 1): imm = M | GROUP_F[op]
        else:                          imm = M

    if f & M:
        if i >= len(b): return None
        modrm = b[i]; i += 1
        mod, rm = modrm >> 6, modrm & 7
        if addrsize == 16:
            if mod == 0 and rm == 6: i += 2
            elif mod == 1: i += 1
            elif mod == 2: i += 2
        elif mod != 3:                        # mod 11 is a register, no sib
            if rm == 4:                       # sib
                if i >= len(b): return None
                sib = b[i]; i += 1
                if mod == 0 and (sib & 7) == 5: i += 4
            if mod == 0 and rm == 5: i += 4
            elif mod == 1: i += 1
            elif mod == 2: i += 4
    if f & MOFF:
        i += 2 if addrsize == 16 else 4
    if imm & I8:  i += 1
    if imm & IZ:  i += 2 if opsize == 16 else 4
    if imm & I16: i += 2
    return i - start if i <= len(b) else None

if __name__ == "__main__":
    import sys
    print("table:", sum(1 for x in T if x != BAD), "of 256 opcodes known")
