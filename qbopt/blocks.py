"""
Which bytes of a module are code, and where control goes.

A linear decode cannot answer this. The code segment opens with a header that is
data, and BC drops jump tables into the middle of it. Reachability can: start
from the entry points the records name, follow control flow, and every byte
reached is code by construction. Everything else is data, and nothing is
guessed at.

The rule that decides it is a runtime name, not an opcode. Calls come back --
that is measured, and treating them as block ends halved what liveness could see
-- but `B$OGTA` does not. `ON GOTO` compiles to a call to it followed by inline
data: a count byte, then that many offset16 words, each one a fixup into this
segment. The runtime reads its own return address to find them. So the byte
after that call is a table, not an instruction, and the only thing that says so
is the EXTDEF the fixup names.
"""

from enum import StrEnum
from dataclasses import dataclass

from qbopt.declen import Insn
from qbopt.declen import decode
from qbopt.module import Module
from qbopt.declen import to_signed

# Runtime routines that do not return to the byte after the call, because their
# arguments are sitting there.
INLINE_TABLE = {"B$OGTA"}

PAD = 0x90

CONDITIONAL = frozenset(range(0x70, 0x80)) | frozenset(range(0xE0, 0xE4)) | frozenset(range(0x0F80, 0x0F90))
UNCONDITIONAL = {0xEB, 0xE9}
RETURNS = {0xC2, 0xC3, 0xCA, 0xCB, 0xCF}
FAR_JUMP = {0xEA}


class Ends(StrEnum):
    FALLS_THROUGH = "falls-through"
    CONDITIONAL = "conditional"
    JUMP = "jump"
    INDIRECT = "indirect"
    RETURN = "return"
    LEAVES = "leaves"
    TABLE = "table"


@dataclass(frozen=True, slots=True)
class CodeMap:
    starts: frozenset[int]
    leaders: frozenset[int]
    tables: tuple[tuple[int, int], ...] = ()
    unreached: tuple[tuple[int, int], ...] = ()


def terminator(insn: Insn) -> Ends:
    """What this instruction does to control flow.

    Not terminators, and the reason matters: 9A and E8 calls, FF /2 and /3
    indirect calls, and CD interrupts all come back.
    """
    match insn.opcode:
        case opcode if opcode in CONDITIONAL:
            return Ends.CONDITIONAL
        case opcode if opcode in UNCONDITIONAL:
            return Ends.JUMP
        case opcode if opcode in RETURNS:
            return Ends.RETURN
        case opcode if opcode in FAR_JUMP:
            return Ends.LEAVES
        case 0xFF if insn.reg in (4, 5):
            return Ends.INDIRECT
        case _:
            return Ends.FALLS_THROUGH


def branch_target(insn: Insn, code: bytes) -> int | None:
    if insn.imm_at is None:
        return None
    raw = int.from_bytes(code[insn.imm_at : insn.imm_at + insn.imm_len], "little")
    return insn.end + to_signed(raw, insn.imm_len)


def inline_table(module: Module, insn: Insn) -> tuple[int, int, list[int]] | None:
    """The extent of the table after a call, and the offsets it holds."""
    if module.calls.get(insn.at) not in INLINE_TABLE:
        return None
    count = module.code[insn.end]
    lo, hi = insn.end, insn.end + 1 + count * 2
    entries = sorted(at for at in module.operands if lo < at < hi)
    return lo, hi, entries


def walk(module: Module, entry: int) -> CodeMap | str:
    """Every byte reachable as an instruction, from the entry points on."""
    entries = sorted({entry} | module.targets | module.publics)

    starts: set[int] = set()
    leaders = set(entries)
    tables: list[tuple[int, int]] = []
    pending = list(entries)

    while pending:
        at = pending.pop()
        while module.start <= at < module.end and at not in starts:
            insn = decode(module.code, at)
            if insn is None:
                return f"the decoder gave up at {at:#x}, reached from an entry point"
            starts.add(at)

            if (table := inline_table(module, insn)) is not None:
                lo, hi, held = table
                if len(held) * 2 + 1 != hi - lo:
                    return f"the table after the call at {insn.at:#x} does not match its count"
                tables.append((lo, hi))
                # It comes back past the table, which is ON GOTO's out-of-range
                # fall-through: ON 0 GOTO ... runs the next statement.
                leaders.add(hi)
                at = hi
                continue

            ends = terminator(insn)
            if ends in (Ends.CONDITIONAL, Ends.JUMP):
                target = branch_target(insn, module.code)
                if target is None or not module.start <= target < module.end:
                    return f"a branch at {insn.at:#x} leaves the module"
                leaders.add(target)
                pending.append(target)
            if ends in (Ends.JUMP, Ends.RETURN, Ends.LEAVES):
                break
            if ends is Ends.INDIRECT:
                return f"an indirect jump at {insn.at:#x} has no computable target"
            if ends is Ends.CONDITIONAL:
                leaders.add(insn.end)
            at = insn.end

    return CodeMap(frozenset(starts), frozenset(leaders), tuple(sorted(tables)), gaps(module, starts, tables))


def gaps(module: Module, starts: set[int], tables: list[tuple[int, int]]) -> tuple[tuple[int, int], ...]:
    """Ranges that reachability never explained, so nothing may be moved across them."""
    covered = bytearray(module.end)
    for at in starts:
        insn = decode(module.code, at)
        if insn is not None:
            covered[at : insn.end] = b"\x01" * insn.length
    for lo, hi in tables:
        covered[lo:hi] = b"\x01" * (hi - lo)

    out, run_from = [], None
    for at in range(module.start, module.end):
        if not covered[at] and run_from is None:
            run_from = at
        elif covered[at] and run_from is not None:
            out.append((run_from, at))
            run_from = None
    if run_from is not None:
        out.append((run_from, module.end))
    return tuple(out)


# How far in to look for where the header stops and code starts. Measured: the
# header's own fields carry fixups up to 0x20 and the earliest operand of an
# instruction is at 0x31, so the boundary is inside this.
HEADER_SEARCH = 0x40


def benign(module: Module, gap: tuple[int, int]) -> list[Insn] | None:
    """Whether a byte range nothing reaches can be left where it is.

    Two kinds turn up. PDS under /Ot pads between procedure bodies with nops.
    And /V /W puts an event-polling call at every statement boundary, including
    ones the optimiser then jumps straight over -- dead code BC emitted and
    never enters. Both are safe to slide, but only if nothing inside them holds
    a self-relative displacement that would need recomputing, so that is the
    test rather than the shape.
    """
    lo, hi = gap
    if set(module.code[lo:hi]) == {PAD}:
        return []
    dead, at = [], lo
    while at < hi:
        insn = decode(module.code, at)
        if insn is None or insn.end > hi:
            return None
        if terminator(insn) in (Ends.CONDITIONAL, Ends.JUMP):
            return None
        dead.append(insn)
        at = insn.end
    return dead if at == hi else None


def explains_the_fixups(module: Module, found: CodeMap, entry: int, dead: list[Insn]) -> bool:
    """Whether every relocated field sits inside an operand of a reached instruction.

    Reachability alone cannot tell a good entry from a bad one: starting inside
    the header drifts into the middle of the first instruction, and the decode
    resynchronises a few bytes later and covers everything after that. The
    fragments it invents are the danger -- `78 56`, the middle of the constant
    0x12345678, decodes as `js`, and retargeting its displacement rewrites the
    constant. The fixups are BC's own map of where operand fields are, so they
    are what says the alignment is right.
    """
    fields = set()
    reached = [insn for at in sorted(found.starts) if (insn := decode(module.code, at)) is not None]
    if len(reached) != len(found.starts):
        return False
    # A real instruction stream tiles. A decode that started inside the header
    # drifts into the middle of the first instruction and reports fragments that
    # overlap it, which is what an entry one or two bytes off looks like.
    if any(earlier.end > later.at for earlier, later in zip(reached, reached[1:], strict=False)):
        return False
    for insn in [*reached, *dead]:
        if insn.disp_at is not None:
            fields |= set(range(insn.disp_at, insn.disp_at + insn.disp_len))
        if insn.imm_at is not None:
            fields |= set(range(insn.imm_at, insn.imm_at + insn.imm_len))
    for lo, hi in found.tables:
        fields |= set(range(lo, hi))
    return all(site in fields for site in module.sites if site >= entry)


def code_map(module: Module) -> CodeMap | str:
    """Where the code is, found rather than assumed.

    Nothing in the records names where module-level code begins: MODEND carries
    no start address, and the header's own pointer is into the middle of the
    module. So the entry is the earliest offset from which reachability explains
    every byte after it. On this corpus that is always 0x30, which is the header
    size the predecessor recorded -- but it is measured here, and a module where
    no offset works is refused rather than guessed at.
    """
    why = "no offset gives a decode the fixups agree with"
    for entry in range(HEADER_SEARCH):
        found = walk(module, entry)
        if isinstance(found, str):
            continue
        dead: list[Insn] = []
        stranded = False
        for gap in found.unreached:
            if gap[0] < entry:
                continue
            leftover = benign(module, gap)
            if leftover is None:
                stranded = True
                why = f"{gap[0]:#x}..{gap[1]:#x} is neither reached nor inert"
                break
            dead += leftover
        if not stranded and explains_the_fixups(module, found, entry, dead):
            return found
    # /V /W puts an event stub in the header region that no record names -- the
    # runtime finds it at a fixed offset -- so those four modules land here.
    return f"no entry point explains the whole segment: {why}"


def instructions(module: Module) -> list[Insn] | str:
    """Every instruction the module actually reaches, in address order."""
    mapped = code_map(module)
    if isinstance(mapped, str):
        return mapped
    return [insn for at in sorted(mapped.starts) if (insn := decode(module.code, at)) is not None]
