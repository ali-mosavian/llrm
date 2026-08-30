"""
Backward liveness for ax/dx/cx/bx, one register at a time.

Built for dx/bx alone -- the two registers lift.py's own restore idiom
(`FIXUP`) puts back after a widened region -- and generalised to all four
once the same question ("is this register's own value still wanted") turned
out to be exactly what a human reading a disassembly by hand keeps asking
about ax and cx too (see tools/dump.py's live.txt output).

Mirrors flags.py's own reads()/writes()/live_in()/live_after() shape, for one
literal register instead of a Flag bitmask. Deliberately NOT ir.py's own
ROOT/root(): ir.py folds every sub-register to its 32-bit parent and --
correctly, for its own purpose of proving arbitrary code motion safe --
counts a partial write (e.g. `mov ax,cx`, touching only ax) as also a *use*
of the root, because the untouched high bits of eax survive and something
later might still want them. That rule answers a different question than
this one: asking whether dx's OWN value is still wanted, rooted to edx,
would report a plain `pop dx` as a read of the old dx and make every restore
look live. This checks the literal 16- and 32-bit registers iced reports,
never rooted -- the same discipline a prior script-driven measurement of
this exact pattern used (tools/residue_census.py), after an earlier manual
disassembly read got it wrong by treating ax/dx as one joint unit instead of
two independent registers.
"""

from dataclasses import dataclass

from iced_x86 import OpAccess
from iced_x86 import Register
from iced_x86 import Register_

from qbopt.declen import INFO
from qbopt.declen import Insn
from qbopt.blocks import Block
from qbopt.declen import READS
from qbopt.flags import CLOBBERS

# The 8-bit halves folded in defensively -- read or written, they are still
# part of the 16/32-bit register's own value, even where nothing in the
# corpus happens to write them today.
GROUP: dict[Register_, frozenset[Register_]] = {
    Register.AX: frozenset({Register.EAX, Register.AX, Register.AL, Register.AH}),
    Register.DX: frozenset({Register.EDX, Register.DX, Register.DL, Register.DH}),
    Register.CX: frozenset({Register.ECX, Register.CX, Register.CL, Register.CH}),
    Register.BX: frozenset({Register.EBX, Register.BX, Register.BL, Register.BH}),
}
# A write through one of these fully overwrites the target; a write to an
# 8-bit half alone does not.
FULL_WIDTH = {
    Register.EAX,
    Register.AX,
    Register.EDX,
    Register.DX,
    Register.ECX,
    Register.CX,
    Register.EBX,
    Register.BX,
}

# Only an unconditional write counts as a kill. declen.WRITES also carries
# COND_WRITE/READ_COND_WRITE (a cmov, a rep-prefixed string op) -- one of
# those that does not fire leaves the register exactly as it was, and
# treating it as a kill can call a still-live restore dead. The read side
# stays declen.READS: a conditional read is still a read, and the
# conservative direction there is to count it.
KILLS = (OpAccess.WRITE, OpAccess.READ_WRITE)


def _touches(insn: Insn, target: Register_) -> tuple[bool, bool]:
    """(reads target, fully overwrites target) for one real instruction."""
    if insn.flow in CLOBBERS:  # a call/interrupt -- conservative both ways
        return True, True
    group = GROUP[target]
    read = write = False
    for one in INFO.info(insn.insn).used_registers():
        if one.register not in group:
            continue
        if one.access in READS:
            read = True
        if one.access in KILLS and one.register in FULL_WIDTH:
            write = True
    return read, write


def reads(block: Block, target: Register_) -> bool:
    """Whether the block reads target before writing it."""
    written = False
    for insn in block.insns:
        r, w = _touches(insn, target)
        if r and not written:
            return True
        if w:
            written = True
    return False


def writes(block: Block, target: Register_) -> bool:
    return any(_touches(insn, target)[1] for insn in block.insns)


def live_in(blocks: list[Block], target: Register_) -> dict[int, bool]:
    """Whether target is live on entry to each block, to a fixed point."""
    known = {block.at for block in blocks}
    live = {block.at: False for block in blocks}
    uses = {block.at: reads(block, target) for block in blocks}
    defs = {block.at: writes(block, target) for block in blocks}

    changing = True
    while changing:
        changing = False
        for block in reversed(blocks):
            out = block.leaves
            for successor in block.succ:
                out = (out or live[successor]) if successor in known else True
            now = uses[block.at] or (out and not defs[block.at])
            if now != live[block.at]:
                live[block.at] = now
                changing = True
    return live


def live_after(block: Block, offset: int, target: Register_, live: dict[int, bool]) -> bool:
    """Whether something reads target after `offset`, without it being rewritten first."""
    out = block.leaves
    for successor in block.succ:
        out = out or live.get(successor, True)

    written = False
    for insn in block.insns:
        if insn.at < offset:
            continue
        r, w = _touches(insn, target)
        if r and not written:
            return True
        if w:
            written = True
    return not written and out


@dataclass(frozen=True, slots=True)
class Liveness:
    ax: dict[int, bool]
    dx: dict[int, bool]
    cx: dict[int, bool]
    bx: dict[int, bool]


def analyse(blocks: list[Block]) -> Liveness:
    return Liveness(
        live_in(blocks, Register.AX),
        live_in(blocks, Register.DX),
        live_in(blocks, Register.CX),
        live_in(blocks, Register.BX),
    )
