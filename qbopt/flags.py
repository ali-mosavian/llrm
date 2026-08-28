"""
Which flags are live, and what each instruction does to them.

BC leaves the *high half's* flags; one 32-bit operation leaves the whole
result's. Measured over 1125 cases: ZF differs on 18.7 per cent of them, PF on
37.2 and AF on 12.4, while CF, SF, OF and the computed value never differ. So a
region whose flags something reads afterwards has to be refused unless nothing
in it writes flags.

That gate was designed into the runtime pass and then lost when its peephole
matcher became a value graph -- set, never tested -- and a `jz` after a widened
`AND` could go the other way. It stays green in every test until it does not,
which is why the accept and the refuse are tested as a pair here.
"""

from enum import IntFlag
from dataclasses import dataclass

from qbopt.declen import Insn
from qbopt.blocks import Block


class Flag(IntFlag):
    NONE = 0
    CF = 0x01
    PF = 0x02
    AF = 0x04
    ZF = 0x08
    SF = 0x10
    OF = 0x20


ALL = Flag.CF | Flag.PF | Flag.AF | Flag.ZF | Flag.SF | Flag.OF

# The three that widening changes the value of. Derived from measurement, not
# asserted against itself: tests/test_flags.py recomputes it.
DIVERGENT = Flag.ZF | Flag.PF | Flag.AF

ARITHMETIC = ALL
NO_CARRY = ALL & ~Flag.CF

# what each condition code reads, by the low nibble of a Jcc, setcc or cmovcc
CONDITION = {
    0x0: Flag.OF,
    0x1: Flag.OF,
    0x2: Flag.CF,
    0x3: Flag.CF,
    0x4: Flag.ZF,
    0x5: Flag.ZF,
    0x6: Flag.CF | Flag.ZF,
    0x7: Flag.CF | Flag.ZF,
    0x8: Flag.SF,
    0x9: Flag.SF,
    0xA: Flag.PF,
    0xB: Flag.PF,
    0xC: Flag.SF | Flag.OF,
    0xD: Flag.SF | Flag.OF,
    0xE: Flag.ZF | Flag.SF | Flag.OF,
    0xF: Flag.ZF | Flag.SF | Flag.OF,
}

# the eight ALU operations, by the top of their opcode grid and by the reg field
# of the 80/81/83 group: adc and sbb are the two that read the carry in
ALU_READS_CARRY = {0x10, 0x18}
GROUP_READS_CARRY = {2, 3}

NOTHING = (Flag.NONE, Flag.NONE)


@dataclass(frozen=True, slots=True)
class Effect:
    reads: Flag
    writes: Flag


def effect(insn: Insn) -> Effect:
    """What this instruction reads and writes.

    An instruction this does not model reads everything and writes nothing.
    Maximal uses, minimal definitions: both directions push the gate toward
    refusing, which is what stops a future simplification defaulting to "no
    effect" and quietly opening the hole.
    """
    op = insn.opcode

    match op:
        case _ if op < 0x40 and (op & 7) < 6:  # the eight ALU operations
            carry = Flag.CF if op & 0x38 in ALU_READS_CARRY else Flag.NONE
            return Effect(carry, ARITHMETIC)
        case 0x80 | 0x81 | 0x83:
            carry = Flag.CF if insn.reg in GROUP_READS_CARRY else Flag.NONE
            return Effect(carry, ARITHMETIC)
        case _ if 0x40 <= op < 0x50:  # inc/dec leave the carry alone
            return Effect(Flag.NONE, NO_CARRY)
        case 0xFE | 0xFF if insn.reg in (0, 1):
            return Effect(Flag.NONE, NO_CARRY)
        case 0x84 | 0x85 | 0xA8 | 0xA9:  # test
            return Effect(Flag.NONE, ARITHMETIC)
        case 0xF6 | 0xF7:
            match insn.reg:
                case 0 | 1:  # test
                    return Effect(Flag.NONE, ARITHMETIC)
                case 2:  # not writes no flags at all
                    return Effect(Flag.NONE, Flag.NONE)
                case _:  # neg, mul, imul, div, idiv
                    return Effect(Flag.NONE, ARITHMETIC)
        case 0xC0 | 0xC1 | 0xD0 | 0xD1 | 0xD2 | 0xD3:  # shifts; rcl and rcr read CF
            carry = Flag.CF if insn.reg in (2, 3) else Flag.NONE
            return Effect(carry, ARITHMETIC)
        case _ if 0x70 <= op < 0x80:
            return Effect(CONDITION[op & 0xF], Flag.NONE)
        case _ if 0x0F80 <= op < 0x0F90:
            return Effect(CONDITION[op & 0xF], Flag.NONE)
        case _ if 0x0F90 <= op < 0x0FA0 or 0x0F40 <= op < 0x0F50:  # setcc, cmovcc
            return Effect(CONDITION[op & 0xF], Flag.NONE)
        case 0x9C | 0x9F:  # pushf, lahf
            return Effect(ALL, Flag.NONE)
        case 0x9D:  # popf
            return Effect(Flag.NONE, ALL)
        case 0x9E:  # sahf leaves OF alone
            return Effect(Flag.NONE, ALL & ~Flag.OF)
        case 0xF5:  # cmc
            return Effect(Flag.CF, Flag.CF)
        case 0xF8 | 0xF9:  # clc, stc
            return Effect(Flag.NONE, Flag.CF)
        case 0x9A | 0xE8 | 0xCD | 0xCC | 0xCE:  # the callee destroys them
            return Effect(Flag.NONE, ALL)
        case 0xFF if insn.reg in (2, 3):  # an indirect call, likewise
            return Effect(Flag.NONE, ALL)
        case 0xE0 | 0xE1:  # loopnz, loopz
            return Effect(Flag.ZF, Flag.NONE)
        case 0xFF if insn.reg in (4, 5, 6):  # indirect jmp, push
            return Effect(Flag.NONE, Flag.NONE)
        case 0xA6 | 0xA7 | 0xAE | 0xAF:  # cmps, scas
            return Effect(Flag.NONE, ARITHMETIC)
        case 0x0FAF | 0x69 | 0x6B:  # imul
            return Effect(Flag.NONE, ARITHMETIC)
        case 0x0FA3 | 0x0FAB | 0x0FB3 | 0x0FBB | 0x0FBA:  # bt and friends
            return Effect(Flag.NONE, ARITHMETIC)
        case 0x0FA4 | 0x0FA5 | 0x0FAC | 0x0FAD:  # shld, shrd
            return Effect(Flag.NONE, ARITHMETIC)
        case _ if op in FLAG_BLIND:
            return Effect(Flag.NONE, Flag.NONE)
        case _:
            return Effect(ALL, Flag.NONE)


# Instructions that touch no flag at all. Everything absent from here and from
# the match above is unmodelled, and so reads everything.
FLAG_BLIND = (
    {0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F}  # mov, lea, pop r/m
    | {0xA0, 0xA1, 0xA2, 0xA3}  # moffs mov
    | {0x86, 0x87}  # xchg
    | set(range(0x50, 0x60))  # push, pop
    | set(range(0xB0, 0xC0))  # mov imm
    | {0x06, 0x07, 0x0E, 0x16, 0x17, 0x1E, 0x1F}  # push/pop segment
    | {0x60, 0x61, 0x68, 0x6A}  # pusha, popa, push imm
    | {0x98, 0x99}  # cbw, cwd
    | set(range(0x90, 0x98))  # nop, xchg ax
    | {0xC4, 0xC5, 0xC8, 0xC9}  # les, lds, enter, leave
    | {0x0FB6, 0x0FB7, 0x0FBE, 0x0FBF}  # movzx, movsx
    | {0xC6, 0xC7}  # mov r/m, imm
    | {0xE9, 0xEB, 0xEA}  # jmp
    | {0xE2, 0xE3}  # loop, jcxz -- unlike loopz and loopnz, these read no flag
    | {0xC2, 0xC3, 0xCA, 0xCB, 0xCF}  # ret, iret
    | {0xA4, 0xA5, 0xAA, 0xAB, 0xAC, 0xAD}  # movs, stos, lods
)


def block_effect(block: Block) -> Effect:
    """What a whole block reads before writing, and what it writes."""
    uses = writes = Flag.NONE
    for insn in block.insns:
        this = effect(insn)
        uses |= this.reads & ~writes
        writes |= this.writes
    return Effect(uses, writes)


def live_in(blocks: list[Block]) -> dict[int, Flag]:
    """The flags live on entry to each block, to a fixed point."""
    at = {block.at: block for block in blocks}
    live = {block.at: Flag.NONE for block in blocks}
    effects = {block.at: block_effect(block) for block in blocks}

    changing = True
    while changing:
        changing = False
        for block in reversed(blocks):
            # Everything this cannot see leaves every flag live. A FUNCTION can
            # return its answer in them -- B$CPI4 proves BC thinks that way.
            out = ALL if block.leaves else Flag.NONE
            for successor in block.succ:
                out |= live.get(successor, ALL) if successor in at else ALL
            found = effects[block.at]
            now = found.reads | (out & ~found.writes)
            if now != live[block.at]:
                live[block.at] = now
                changing = True
    return live


def live_after(block: Block, offset: int, live: dict[int, Flag]) -> Flag:
    """The flags something reads after `offset`, without them being rewritten first."""
    out = ALL if block.leaves else Flag.NONE
    for successor in block.succ:
        out |= live.get(successor, ALL)

    needed = written = Flag.NONE
    for insn in block.insns:
        if insn.at < offset:
            continue
        this = effect(insn)
        needed |= this.reads & ~written
        written |= this.writes
        if written & ALL == ALL:
            return needed
    return needed | (out & ~written)
