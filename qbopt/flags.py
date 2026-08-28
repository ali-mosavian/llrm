"""
Which flags are live.

BC leaves the *high half's* flags; one 32-bit operation leaves the whole
result's. Measured over 1125 cases: ZF differs on 18.7 per cent of them, PF on
37.2 and AF on 12.4, while CF, SF, OF and the computed value never differ. So a
region whose flags something reads afterwards has to be refused unless nothing
in it writes flags.

That gate was designed into the runtime pass and then lost when its peephole
matcher became a value graph -- set, never tested -- and a `jz` after a widened
`AND` could go the other way. It stays green in every test until it does not,
which is why the accept and the refuse are tested as a pair here.

What each instruction does to the flags comes from iced. It used to be a table
written here, which was a list of qbopt's guesses about an architecture someone
else has already modelled -- and it had no notion of a flag left *undefined*,
which is as dangerous to read as one left wrong.
"""

from enum import IntFlag

from iced_x86 import RflagsBits
from iced_x86 import FlowControl

from qbopt.declen import Insn
from qbopt.blocks import Block


class Flag(IntFlag):
    NONE = 0
    CF = RflagsBits.CF
    PF = RflagsBits.PF
    AF = RflagsBits.AF
    ZF = RflagsBits.ZF
    SF = RflagsBits.SF
    OF = RflagsBits.OF


ALL = Flag.CF | Flag.PF | Flag.AF | Flag.ZF | Flag.SF | Flag.OF

# The three whose value widening changes. Derived from measurement, not asserted
# against itself: tests/test_flags.py recomputes it.
DIVERGENT = Flag.ZF | Flag.PF | Flag.AF


# iced models the instruction, and a call does not itself write a flag. The
# callee does, whatever it is: BC's runtime routines are ordinary code and leave
# the flags however they end. B$CPI4 is the case that proves it deliberately --
# it returns its answer in them. So nothing set before a call survives it.
CLOBBERS = {FlowControl.CALL, FlowControl.INDIRECT_CALL, FlowControl.INTERRUPT}


def written_by(insn: Insn) -> Flag:
    return ALL if insn.flow in CLOBBERS else Flag(insn.writes & ALL)


def reads(block: Block) -> Flag:
    """What the block reads before writing it."""
    uses = written = Flag.NONE
    for insn in block.insns:
        uses |= Flag(insn.reads & ALL) & ~written
        written |= written_by(insn)
    return uses


def writes(block: Block) -> Flag:
    found = Flag.NONE
    for insn in block.insns:
        found |= written_by(insn)
    return found


def live_in(blocks: list[Block]) -> dict[int, Flag]:
    """The flags live on entry to each block, to a fixed point."""
    known = {block.at for block in blocks}
    live = {block.at: Flag.NONE for block in blocks}
    uses = {block.at: reads(block) for block in blocks}
    defs = {block.at: writes(block) for block in blocks}

    changing = True
    while changing:
        changing = False
        for block in reversed(blocks):
            # Everything this cannot see leaves every flag live. A FUNCTION can
            # return its answer in them -- B$CPI4 proves BC thinks that way.
            out = ALL if block.leaves else Flag.NONE
            for successor in block.succ:
                out |= live[successor] if successor in known else ALL
            now = uses[block.at] | (out & ~defs[block.at])
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
        needed |= Flag(insn.reads & ALL) & ~written
        written |= written_by(insn)
        if written == ALL:
            return needed
    return needed | (out & ~written)
