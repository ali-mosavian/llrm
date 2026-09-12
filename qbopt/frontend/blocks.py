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
from dataclasses import replace
from dataclasses import dataclass

from iced_x86 import Code
from iced_x86 import Register
from iced_x86 import FlowControl

from qbopt.objectfile import omf
from qbopt.frontend import fppatches
from qbopt.frontend.declen import Insn
from qbopt.frontend.declen import decode
from qbopt.objectfile.module import Space
from qbopt.objectfile.module import Module
from qbopt.objectfile.module import family
from qbopt.objectfile.module import defines

# Runtime routines that do not return to the byte after the call, because their
# arguments are sitting there.
# The one routine that reads a table laid inline after its own call site.
# Established from the shipped libraries rather than inferred -- gosub.asm
# in BCOM45.LIB, BCL71ENR.LIB and VBDCL10E.LIB, read with tools/libdump.py.
# The implementations differ; the QB form below shows the table protocol:
#
#   lds  si,[bp+2]     si = the return address, which IS the table
#   lodsb              al = the count, si now on the entries
#   mov  dl,al / shl dx,1 / add dx,si    dx = past the table
#   mov  cx,[bx+si]    cx = entry[index-1]
#   cmp  al,bl / jbe   out of range falls through to dx
#   push bx / push dx  and a far return goes to whichever was chosen
#
# So an entry is a two-byte offset into this same segment, the count is one
# byte in front of them. Selectors 0 or count+1..255 fall through; values
# above 255 enter B$FrameFC instead. These are normal CFG successors only.
INLINE_TABLE = {"B$OGTA"}

PAD = 0x90


class Ends(StrEnum):
    FALLS_THROUGH = "falls-through"
    CONDITIONAL = "conditional"
    JUMP = "jump"
    INDIRECT = "indirect"
    RETURN = "return"
    LEAVES = "leaves"
    TABLE = "table"


# What iced calls it, and what it means for a block. The one that matters is
# that a call is NEXT-like: it comes back, and treating it as an end halved what
# liveness could see -- 49 of 102 blocks.
ENDS = {
    FlowControl.NEXT: Ends.FALLS_THROUGH,
    FlowControl.CALL: Ends.FALLS_THROUGH,
    FlowControl.INTERRUPT: Ends.FALLS_THROUGH,
    FlowControl.INDIRECT_CALL: Ends.FALLS_THROUGH,
    FlowControl.CONDITIONAL_BRANCH: Ends.CONDITIONAL,
    FlowControl.UNCONDITIONAL_BRANCH: Ends.JUMP,
    FlowControl.INDIRECT_BRANCH: Ends.INDIRECT,
    FlowControl.RETURN: Ends.RETURN,
    FlowControl.EXCEPTION: Ends.LEAVES,
    FlowControl.XBEGIN_XABORT_XEND: Ends.LEAVES,
}


@dataclass(frozen=True, slots=True)
class CodeMap:
    starts: frozenset[int]
    leaders: frozenset[int]
    tables: tuple[tuple[int, int], ...] = ()
    unreached: tuple[tuple[int, int], ...] = ()
    procedures: frozenset[int] = frozenset()


def terminator(insn: Insn, module: Module | None = None) -> Ends:
    """What this instruction does to control flow."""
    if (
        module is not None
        and module.calls.get(insn.at) == "B$RETA"
        and family(module.records) in ("pds71", "vbdos")
        and "B$RETA" not in defines(module.records, module.seg)
    ):
        # gosub.asm discards this call's return IP, then returns to the
        # GOSUB continuation or tail-jumps through B$EVTRET for an event.
        return Ends.LEAVES
    ends = ENDS.get(insn.flow, Ends.LEAVES)
    # a far jump goes somewhere this module cannot follow
    if ends is Ends.JUMP and insn.target is None:
        return Ends.LEAVES
    return ends


def inline_table(module: Module, insn: Insn) -> tuple[int, int, list[int]] | None:
    """The extent of the table after a call, and the offsets it holds."""
    if module.calls.get(insn.at) not in INLINE_TABLE:
        return None
    count = module.code[insn.end]
    lo, hi = insn.end, insn.end + 1 + count * 2
    entries = sorted(at for at in module.operands if lo < at < hi)
    return lo, hi, entries


def dispatch_targets(module: Module, insn: Insn) -> tuple[int, ...] | None:
    if insn.end >= min(module.end, len(module.code)):
        return None
    table = inline_table(module, insn)
    if table is None:
        return None
    lo, hi, fields = table
    if hi > min(module.end, len(module.code)) or fields != list(range(lo + 1, hi, 2)):
        return None
    entries = [module.operands[at] for at in fields]
    if any(
        entry.space is not Space.SEGMENT or entry.index != module.seg or not module.start <= entry.disp < module.end
        for entry in entries
    ):
        return None
    return tuple(entry.disp for entry in entries)


# MODULE_CODE in the QuickBASIC 4.5 runtime's addr.inc: a signature word, an
# eight-byte module name and nineteen more words, 48 bytes. The runtime calls
# the offset past it O_ENT, and rtinit.asm says the beginning of the user's
# code is at that fixed offset from the module header.
SIGNATURES = (b"bl", b"bm", b"br")
ENTRY = 0x30

# U_FLAG, the header's last word, records the switches BC was given. Under /V
# or /W the module opens with a jump over a sixteen-byte event-poll routine
# that only the runtime enters, at a fixed offset the same way the code is --
# so nothing falls into it and reachability cannot find it unaided. QuickBASIC
# 4.5 sets the bits and emits no stub; PDS and VBDOS emit one, in 14 of the
# corpus's objects.
U_FLAG = 0x2E
EVENTS = 0x400 | 0x800  # u_sw_v, u_sw_w


def has_header(module: Module) -> bool:
    return module.code[:2] in SIGNATURES


def event_enabled(module: Module) -> bool:
    """The compiler requested event checks, whether or not it emitted a stub."""
    return (
        has_header(module)
        and len(module.code) >= U_FLAG + 2
        and bool(int.from_bytes(module.code[U_FLAG : U_FLAG + 2], "little") & EVENTS)
    )


# CMP word [b$EVTFLG],0 / JNE past / RET / POP AX / PUSH CS / PUSH AX / JMP FAR B$EVK1
EVENT_ADAPTER = (
    (Code.CMP_RM16_IMM8, Code.CMP_RM16_IMM16),
    (Code.JNE_REL8_16, Code.JNE_REL16),
    (Code.RETNW,),
    (Code.POP_R16,),
    (Code.PUSHW_CS,),
    (Code.PUSH_R16,),
    (Code.JMP_PTR1616,),
)


def event_stub(module: Module) -> int | None:
    """Where the event-poll routine starts, in a module that carries one.

    Identified by its instructions and relocations, not its place or
    encoding: BC puts it just past the entry jump, a rebuilt segment may lay
    it out anywhere and re-encode its compare.
    """
    if not event_enabled(module):
        return None
    names = omf.externals(module.records)
    fields = {fixup.offset: fixup for fixup in omf.fixups(module.records) if fixup.seg == module.seg}

    def named(at: int, loc: int, name: str) -> bool:
        fixup = fields.get(at)
        return (
            fixup is not None
            and fixup.target == "external"
            and fixup.disp == 0
            and not fixup.selfrel
            and fixup.loc == loc
            and names[fixup.index] == name
        )

    for flag in sorted(at for at in fields if named(at, omf.LOC_OFF16, "b$EVTFLG")):
        insns, at = [], flag - 2
        for codes in EVENT_ADAPTER:
            one = decode(module.code, at)
            if one is None or one.insn.code not in codes:
                break
            insns.append(one)
            at = one.end
        else:
            compare, skip, _ret, pop, _cs, push, far = insns
            if (
                compare.insn.immediate(1) == 0
                and skip.insn.near_branch_target == pop.at
                and pop.insn.op0_register == push.insn.op0_register == Register.AX
                and named(far.at + 1, omf.LOC_PTR32, "B$EVK1")
                and sum(insns[0].at <= site < far.end for site in fields) == 2
            ):
                return insns[0].at
    return None


def local_call_target(module: Module, insn: Insn) -> int | None:
    if insn.flow != FlowControl.CALL or insn.target is None or any(insn.at <= site < insn.end for site in module.sites):
        return None
    return insn.target if module.start <= insn.target < module.end else None


def walk(module: Module, entry: int, extra_entries: frozenset[int] = frozenset()) -> CodeMap | str:
    """Every byte reachable as an instruction, from the entry points on."""
    seeds = {entry} | module.targets | module.publics | extra_entries
    if (stub := event_stub(module)) is not None:
        seeds.add(stub)
    entries = sorted(seeds)

    starts: set[int] = set()
    leaders = set(entries)
    declared = statement_table(module)
    tables: list[tuple[int, int]] = [declared] if declared is not None else []
    pending = list(entries)

    while pending:
        at = pending.pop()
        while module.start <= at < module.end and at not in starts:
            if declared is not None and declared[0] <= at < declared[1]:
                break
            insn = decode(module.code, at)
            if insn is None:
                return f"the decoder gave up at {at:#x}, reached from an entry point"
            starts.add(at)

            # A reachable private near call identifies code, but its callee
            # is not a CFG successor: execution also continues after the call.
            if (target := local_call_target(module, insn)) is not None:
                leaders.add(target)
                pending.append(target)

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

            ends = terminator(insn, module)
            if ends in (Ends.CONDITIONAL, Ends.JUMP):
                target = insn.target
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

    return CodeMap(
        frozenset(starts),
        frozenset(leaders),
        tuple(sorted(tables)),
        gaps(module, starts, tables),
        extra_entries,
    )


def native_gap_entry(
    module: Module,
    found: CodeMap,
    gap: tuple[int, int],
    entry: int,
    extra_entries: frozenset[int],
) -> int | None:
    """A disconnected, frame-based C procedure that exactly explains a gap.

    Stripped Borland objects do not name an unreferenced static procedure in
    PUBDEF, LINNUM or a debug segment.  Its ABI entry still does: PUSH BP;
    MOV BP,SP.  The byte shape alone is not enough, so accept it only when a
    walk from that entry stays inside this one previously unexplained range,
    every path is closed by a return, and anything it leaves behind is inert.
    """
    lo, hi = gap
    push = decode(module.code, lo)
    establish = decode(module.code, push.end) if push is not None else None
    if not (
        push is not None
        and establish is not None
        and push.code == Code.PUSH_R16
        and push.insn.op0_register == Register.BP
        and establish.code == Code.MOV_R16_RM16
        and establish.insn.op0_register == Register.BP
        and establish.insn.op1_register == Register.SP
        and push.end == establish.at
    ):
        return None

    candidate = walk(module, entry, extra_entries | {lo})
    if isinstance(candidate, str):
        return None
    added = candidate.starts - found.starts
    if not added or any(not lo <= at < hi for at in added):
        return None

    candidate_blocks = [block for block in partition(module, candidate) if block.at in added]
    if not candidate_blocks or any(
        any(successor not in added for successor in block.succ) or (not block.succ and block.ends is not Ends.RETURN)
        for block in candidate_blocks
    ):
        return None
    if any(
        leftover is None
        for remaining in candidate.unreached
        if remaining[0] < hi and remaining[1] > lo
        for leftover in [benign(module, (max(lo, remaining[0]), min(hi, remaining[1])))]
    ):
        return None
    return lo


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


# A table has at least this many entries. Three is what ON GOTO's smallest form
# has, and a run that short at a constant stride does not happen by accident.
SHORTEST_TABLE = 3


def statement_table(module: Module) -> tuple[int, int] | None:
    """OF_STA points to an address/line table terminated by a zero word."""
    if not has_header(module):
        return None
    ref = module.operands.get(0x0A)
    if (
        ref is None
        or ref.space is not Space.SEGMENT
        or ref.index != module.seg
        or module.code[0x0A:0x0C] != b"\x00\x00"
    ):
        return None
    start = at = ref.disp
    if at < ENTRY:
        return None
    while at + 2 <= module.end:
        address = module.operands.get(at)
        if address is None:
            if module.code[at : at + 2] == b"\x00\x00" and not any(at <= site < at + 2 for site in module.sites):
                return start, at + 2
            return None
        if (
            address.space is not Space.SEGMENT
            or address.index != module.seg
            or at + 4 > module.end
            or module.code[at : at + 2] != b"\x00\x00"
            or any(at < site < at + 4 for site in module.sites)
        ):
            return None
        at += 4
    return None


def unexplained_tables(module: Module, fields: set[int], entry: int) -> tuple[tuple[int, int], ...]:
    """Runs of relocations no instruction accounts for, which are a table.

    BC puts more than one kind in the code segment. ON GOTO's is found by the
    call in front of it; this is the other -- under /X, a map from statement to
    code offset so RESUME can find its way back, thirty-seven entries in a
    program with one error handler. It sits past the end of the code and the
    walk falls into it, decoding zeros as instructions, so reachability cannot
    be what finds it.

    What finds it is the fixups: a run of them at a constant stride that no
    operand field explains. Misalignment does not produce that.
    """
    # the header's own fields are below the entry and already exempt
    missing = sorted(site for site in module.sites if site >= entry and site not in fields)
    found, start = [], 0
    while start < len(missing):
        stop, stride = start + 1, None
        while stop < len(missing):
            step = missing[stop] - missing[stop - 1]
            if stride is None:
                stride = step
            elif step != stride:
                break
            stop += 1
        if stride is not None and stop - start >= SHORTEST_TABLE:
            found.append((missing[start], missing[stop - 1] + stride))
        start = stop
    return tuple(found)


def benign(module: Module, gap: tuple[int, int]) -> list[Insn] | None:
    """Whether a byte range nothing reaches can be left where it is.

    Two kinds turn up. PDS under /Ot pads between procedure bodies with nops.
    And /V /W puts an event-polling call at every statement boundary, including
    ones the optimiser then jumps straight over -- dead code BC emitted and
    never enters. BC also leaves branches from loop forms that an earlier
    simplification made unreachable. All are safe to slide as opaque bytes:
    reachability proves no instruction can execute an internal relative
    displacement, while a public, fixup or live branch target into the span is
    already an entry and therefore keeps it out of a gap. The test here is only
    that the whole span is instruction-shaped, rather than accepting arbitrary
    data because it happens to be unreachable.
    """
    lo, hi = gap
    if set(module.code[lo:hi]) == {PAD}:
        return []
    dead, at = [], lo
    while at < hi:
        insn = decode(module.code, at)
        if insn is None or insn.end > hi:
            return None
        dead.append(insn)
        at = insn.end
    return dead if at == hi else None


def operand_fields(module: Module, found: CodeMap, dead: list[Insn]) -> set[int] | None:
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
        return None
    # A real instruction stream tiles. A decode that started inside the header
    # drifts into the middle of the first instruction and reports fragments that
    # overlap it, which is what an entry one or two bytes off looks like.
    if any(earlier.end > later.at for earlier, later in zip(reached, reached[1:], strict=False)):
        return None
    for insn in [*reached, *dead]:
        if insn.disp_at is not None:
            fields |= set(range(insn.disp_at, insn.disp_at + insn.disp_len))
        if insn.imm_at is not None:
            fields |= set(range(insn.imm_at, insn.imm_at + insn.imm_len))
    for lo, hi in found.tables:
        fields |= set(range(lo, hi))
    return fields


def code_map(module: Module) -> CodeMap | str:
    """Where the code is: after the header, whose shape the runtime defines.

    Nothing in the OMF records names it -- MODEND's start-address bit is clear
    on every object here. The BASIC runtime names it instead: MODULE_CODE in
    QuickBASIC 4.5's runtime/inc/addr.inc is a fixed structure whose fields sum
    to 48, and the offset past it is O_ENT = 48. Its first field is a signature
    word, 'bl' for a BCOM module and 'bm' or 'br' for a BRUN one, so the layout
    is checked rather than assumed. All 110 objects in the corpus carry one, as
    do all 15 modules of qb-qrender.

    Searching for the entry instead is what the signature replaces, and it was
    wrong on 22 of those 110: a /G3 module opens with a 66-prefixed store, and
    starting one byte into it puts the displacement field at the same offset, so
    0x30 and 0x31 explain exactly the same fixups and the tie-break took the
    later one. The search is still here for a module with no signature, where
    there is nothing else to lean on.
    """
    why = "no offset gives a decode the fixups agree with"
    best: tuple[int, int, CodeMap] | None = None

    for entry in [ENTRY] if has_header(module) else range(HEADER_SEARCH):
        native = not has_header(module) and omf.code_segment(module.records) is not None
        extra_entries: set[int] = set()
        while True:
            found = walk(module, entry, frozenset(extra_entries))
            if isinstance(found, str):
                break
            stranded_gap = next(
                (gap for gap in found.unreached if gap[0] >= entry and benign(module, gap) is None),
                None,
            )
            if stranded_gap is None:
                break
            discovered = (
                native_gap_entry(module, found, stranded_gap, entry, frozenset(extra_entries)) if native else None
            )
            if discovered is None:
                why = f"{stranded_gap[0]:#x}..{stranded_gap[1]:#x} is neither reached nor inert"
                found = "unexplained code"
                break
            extra_entries.add(discovered)
        if isinstance(found, str):
            continue
        dead = [insn for gap in found.unreached if gap[0] >= entry for insn in (benign(module, gap) or ())]
        fields = operand_fields(module, found, dead)
        if fields is None:
            continue
        patches = fppatches.sites(module, found.starts)
        tables = unexplained_tables(module, fields | patches, entry)
        for lo, hi in tables:
            fields |= set(range(lo, hi))
        if not all(site in fields or site in patches for site in module.sites if site >= entry):
            continue
        if tables:
            covered = {at for lo, hi in tables for at in range(lo, hi)}
            found = replace(
                found,
                starts=frozenset(found.starts - covered),
                tables=tuple(sorted([*found.tables, *tables])),
            )
        # Score by how many relocated fields the decode accounts for. An entry
        # too early reads header bytes as code; one too late skips real code and
        # leaves its operands unexplained. Both are accepted by the checks above,
        # because each exempts whatever lies before the entry it was given. The
        # count is what separates them, and the largest entry breaks the tie, so
        # the least data gets decoded.
        explained = sum(1 for site in module.sites if site in fields or site in patches)
        if best is None or (explained, entry) > (best[0], best[1]):
            best = (explained, entry, found)

    if best is not None:
        return best[2]

    # /V /W puts an event stub in the header region that no record names -- the
    # runtime finds it at a fixed offset -- so those four modules land here.
    return f"no entry point explains the whole segment: {why}"


def decoded_instruction(module: Module, at: int) -> Insn | None:
    """Decode with the FP emulator's segment protocol restored."""
    insn = decode(module.code, at)
    if (
        insn is not None
        and module.code[at : at + 2] == b"\xcd\x3c"
        and insn.insn.code != Code.INT_IMM8
        and "FIDRQQ" in omf.externals(module.records)
    ):
        # The emulator patches CD 3C D9 07 into 90 26 D9 07: ES, not DS.
        # Read out of QuickBASIC 4.5's own deedlines at 0824:A3F2, so the
        # protocol is the emulator's and not one dialect's -- gating it on
        # vbdos left every QB float access reading a segment nothing set,
        # and `dead` then deleted the `mov es,[si+2]` before it.
        # Keep file offsets and length; only the virtual instruction changes.
        native = insn.insn.copy()
        native.segment_prefix = Register.ES
        return replace(insn, insn=native)
    return insn


def instructions(module: Module) -> list[Insn] | str:
    """Every instruction the module actually reaches, in address order."""
    mapped = code_map(module)
    if isinstance(mapped, str):
        return mapped
    return [insn for at in sorted(mapped.starts) if (insn := decoded_instruction(module, at)) is not None]


@dataclass(frozen=True, slots=True)
class Block:
    at: int
    end: int
    insns: tuple[Insn, ...]
    ends: Ends
    succ: tuple[int, ...]

    @property
    def leaves(self) -> bool:
        """Whether control goes somewhere this cannot see."""
        return self.ends in (Ends.RETURN, Ends.LEAVES, Ends.INDIRECT) or not self.succ


def partition(module: Module, mapped: CodeMap) -> list[Block]:
    """The reached instructions cut into basic blocks, with their successors."""
    reached = {at: insn for at in mapped.starts if (insn := decoded_instruction(module, at)) is not None}
    out: list[Block] = []
    run: list[Insn] = []

    for at in sorted(reached):
        insn = reached[at]
        if run and (at in mapped.leaders or run[-1].end != at):
            out.append(_close(module, run, mapped))
            run = []
        run.append(insn)
        if terminator(insn, module) is not Ends.FALLS_THROUGH or _table_at(module, mapped, insn):
            out.append(_close(module, run, mapped))
            run = []
    if run:
        out.append(_close(module, run, mapped))
    return out


def _table_at(module: Module, mapped: CodeMap, insn: Insn) -> tuple[int, int] | None:
    return next((table for table in mapped.tables if table[0] == insn.end), None)


def _close(module: Module, run: list[Insn], mapped: CodeMap) -> Block:
    last = run[-1]
    ends = terminator(last, module)
    succ: list[int] = []

    if (table := _table_at(module, mapped, last)) is not None:
        targets = dispatch_targets(module, last)
        succ = [*(sorted(module.targets) if targets is None else targets), table[1]]
        ends = Ends.TABLE
    elif ends in (Ends.CONDITIONAL, Ends.JUMP):
        target = last.target
        succ = [target] if target is not None else []
        if ends is Ends.CONDITIONAL:
            succ.append(last.end)
    elif ends is Ends.FALLS_THROUGH:
        succ = [last.end]

    inside = [at for at in succ if module.start <= at < module.end]
    return Block(run[0].at, last.end, tuple(run), ends, tuple(sorted(set(inside))) if len(inside) == len(succ) else ())


def block_at(blocks: list[Block], offset: int) -> Block | None:
    return next((block for block in blocks if block.at <= offset < block.end), None)
