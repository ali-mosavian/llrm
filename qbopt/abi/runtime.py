"""
What a call into BC's runtime can do to the caller, routine by routine.

The optimising backend -- register allocation, CSE, dead-store elimination,
keeping a hot long in a register across a statement -- has to answer the same
question at every `call far B$xxxx`: can the callee read or write the caller's
variables, and which registers come back intact. Today the answer is "assume
the worst" at every one of them, which is correct and refuses essentially all
motion across a call. This is the table that lets a caller ask instead.

**The two wrong answers do not cost the same.** A contract that wrongly calls
a routine clean licenses keeping a value in a register the callee overwrote in
memory, or dropping a store the callee reads: silent, data-dependent
corruption in the emitted program, of the kind that surfaces as a fixture
printing a different number for no reason anyone can trace. A contract that
wrongly calls a routine dirty costs one refused region. So anything that could
not be established from the QuickBASIC 4.5 runtime source at
~/work/ms/msdos_60/45/runtime gets WORST -- writes anything, reads anything,
clobbers everything, control unknown -- and says so with `established=False`.
Nothing here is guessed and nothing is omitted: `contract()` answers for a
name it has never heard of exactly the way it answers for one whose source is
not on disk.

Registers are named 16-bit because the runtime is 8086 code, and a clobber has
to be read as killing the whole 32-bit register. Nothing establishes that the
high half of eax survives a routine that writes ax -- the C helpers behind
B$MUI4/B$DVI4/B$RMI4 are not in the local tree at all, and the rest was
assembled years before a 386 was a target. `preserves()` is the complement of
`clobbers`, so the two cannot disagree.

Flags are one of those registers here. flags.py already treats *every* call as
writing all six and that gate is not for this table to relax; two routines
below (B$DSEG, B$FERR) provably leave them alone, recorded because it is true
rather than because anything should act on it. B$CPI4 is the opposite case and
the reason flags.py is written that way: it returns its answer in them.

`enters_user_code` and `raises_error` are deliberately separate. The first
means the routine reaches user BASIC on an ordinary path -- B$EVCK GOSUBs an
ON TIMER/ON KEY handler, B$OEGA and B$RESN transfer into ON ERROR handling --
and makes every other field meaningless, because the handler can assign any
module-level variable it can name. The second means the routine can raise a
trappable BASIC error, which reaches user code only in a program that has an
ON ERROR handler at all. `error_handling` marks the three routines whose
presence proves the module has one; a body containing any of them turns every
`raises_error` call in it into a possible transfer to user code, which is a
module-level property no single call's contract can carry. `barrier()` folds
all three together for a pass that just wants to know whether to refuse.

The user has decided the optimiser will not support /V or /W (event trapping)
or /X (resumable error handling), so the routines that reach user code do not
need a precise contract -- they need to be unmistakably marked, which they are.

Floating point is not modelled. B$FVAL returns its answer in the DAC through
the mathpack and is recorded only for what it does to memory and registers.

Two headers in the runtime source state a clobber set that is not the one the
body has, in both directions, and both are recorded in `documented`:

  - helpi4.asm gives B$CPI4 "Uses: ax,cx,dx,bx" and the body touches nothing
    but the flags. calls.py already depends on that and says why.
  - ifout.asm glosses the runtime's "Uses: Per convention" as "DS, ES, SI, DI,
    BP preserved", while prnval.asm says of the same conversion routine
    "Currently B$FOUTBX changes all registers except BP". They cannot both be
    true; the conservative one wins for every routine that reaches B$FOUTBX.
"""

import tomllib
from enum import IntEnum
from enum import StrEnum
from pathlib import Path
from dataclasses import field
from dataclasses import replace
from typing import TYPE_CHECKING
from dataclasses import dataclass

if TYPE_CHECKING:
    from qbopt.objectfile.module import Module

from qbopt.frontend.blocks import INLINE_TABLE


class Reg(StrEnum):
    AX = "ax"
    BX = "bx"
    CX = "cx"
    DX = "dx"
    SI = "si"
    DI = "di"
    BP = "bp"
    SP = "sp"
    DS = "ds"
    ES = "es"
    FLAGS = "flags"


EVERY = frozenset(Reg)

# cmacros' own PL/M convention: cBegin saves the routine's declared list and
# cEnd restores it, so a save list is a preservation guarantee whatever the
# body and its callees do (inc/cmacros.inc, mPush/mPop around the frame).
PER_CONVENTION = frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.FLAGS})


# Ordered by how much it concedes, so "can this write something the caller can
# name" is a comparison rather than a second table.
class Memory(IntEnum):
    NONE = 0
    ARGUMENTS = 1  # only the values pushed for this call, popped before it returns
    STRINGS = 2  # + any string descriptor or string data, anywhere
    # Its own data, and anything the caller handed it a pointer to. GCC's
    # ipa-modref splits a callee's effects the same way: writes at a fixed
    # address, and writes through parameter N. `ANY` says only "writes
    # memory", which is true of every routine here and tells a caller
    # nothing -- a global cannot be kept in a register across a PRINT, and
    # every program in the suite prints.
    #
    # Measured, not assumed. tools/runtime_writes.py reads a linked image:
    # over B_NBODY, B_ARRIDX and B_MATRIX, the runtime makes 287 writes to
    # a fixed address in DGROUP and not one of them names a cell in
    # BC_DATA, the segment BC puts a program's variables in. Nor does any
    # data-segment fixup in the corpus hand the runtime a pointer into it:
    # 151 are BC_CN -> BC_CN and one per program is BC_SA -> its code.
    OWN = 3
    ANY = 4


class Control(StrEnum):
    RETURNS = "returns"
    INLINE_TABLE = "inline-table"  # comes back somewhere data after the call selects
    NEVER = "never"
    UNKNOWN = "unknown"


@dataclass(frozen=True, slots=True)
class Contract:
    name: str
    cleanup: int | None  # bytes the callee pops off the caller's stack
    control: Control
    enters_user_code: bool
    raises_error: bool
    error_handling: bool
    writes: Memory
    reads: Memory
    clobbers: frozenset[Reg]
    established: bool
    evidence: str
    documented: frozenset[Reg] | None = field(default=None)
    # Registers the caller has to have set before the call. None is not
    # "none" -- it is "not established", and reads as every register, the
    # same way an unestablished contract clobbers every one. The asymmetry
    # is the point: over-stating a read only keeps a value alive, while
    # under-stating one deletes the instruction that produced it. B$FILD
    # takes its long in dx:ax, nothing recorded that, and removing the
    # moves that set it up printed FADD= 918528 for 1049600.
    inputs: frozenset[Reg] | None = field(default=None)


def worst(name: str) -> Contract:
    return Contract(
        name=name,
        cleanup=None,
        control=Control.UNKNOWN,
        enters_user_code=True,
        raises_error=True,
        error_handling=False,
        writes=Memory.ANY,
        reads=Memory.ANY,
        clobbers=EVERY,
        established=False,
        evidence="not established from the QuickBASIC 4.5 runtime source",
    )


def preserves(routine: Contract) -> frozenset[Reg]:
    return EVERY - routine.clobbers


# The order a routine's declared inputs are listed in. `Contract.inputs` is
# a set -- which register, not which position -- and the raise and the
# lowering both have to agree on a slot per input or they would pair an
# argument with somebody else's register. Any total order does; this is
# `Reg`'s own declaration order, so a register added there cannot be left
# out of a list written somewhere else.
SLOTS: tuple[Reg, ...] = tuple(Reg)


# What a routine's contract is under one toolchain, where the routines
# differ. Keyed by the compiler's own name for itself, which module.py
# reads off COMENT 0x00 -- the family is a fact about the object, and it
# reaches this map and nothing else.
#
# B$ENRA takes the frame size in cx. Disassembled from the linked image:
# PDS 7.1's is at 0x1d35 and QuickBASIC 4.5's at 0x211d, both reading cx
# through `push cx` and `sub sp,cx` before writing it, with no unresolved
# edge on any path from entry to the `jmp far` that returns. VBDOS's, at
# 0x397, also reads bx -- `or bx,bx` gates `call 02f7:0002` -- and that
# helper reaches `call far [di+24h]`, which no disassembly can follow, so
# VBDOS stays at the worst case.
VARIANTS: "dict[tuple[str, str], Contract]" = {}

for _family in ("qb45", "pds71", "vbdos"):
    VARIANTS[("B$LINA", _family)] = replace(
        worst("B$LINA"), cleanup=0, established=True,
        inputs=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
        evidence=(
            "Shipped dbtrace.asm 1:0000..002b in BCOM45/BCL71ENR/VBDCL10E: "
            "both ordinary exits POP BP; RETF without stack arguments. "
            "Bound GP inputs conservatively by all six registers. The /D keyboard "
            "poll installed by DBINI clears arithmetic flags before dispatch; "
            "LINA tests its own trace flag after BREAK_CHK. Trace output, break "
            "handling and indirect dependencies retain worst-case effects."
        ),
    )
    VARIANTS[("B$HARY", _family)] = Contract(
        name="B$HARY", cleanup=None, control=Control.RETURNS,
        enters_user_code=False, raises_error=True, error_handling=False,
        writes=Memory.OWN, reads=Memory.ANY,
        clobbers=frozenset({Reg.BX, Reg.ES, Reg.FLAGS}), established=True,
        inputs=frozenset({Reg.BX}),
        evidence=(
            "Shipped dynamic.asm 1:00dc (QB) / hugearr.asm 1:0004 (PDS/VBDOS): "
            "BX descriptor and stack indices/rank; returns ES:BX. Entry pushes "
            "BP,AX,CX,DX,SI,DI and the normal exit restores them. XOR SI,SI kills "
            "incoming flags. Bounds/rank/unallocated failures enter ERR_BS. "
            "CleanStack removes 2*(rank+1) bytes through runtime scratch; cleanup "
            "is deliberately unknown until the call site's rank is established. "
            "Descriptor reads and error handling are not pure."
        ),
    )

for _family in ("pds71", "vbdos"):
    VARIANTS[("B$RETA", _family)] = replace(
        worst("B$RETA"),
        inputs=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
        evidence=(
            "Shipped PDS/VBDOS gosub.asm RETA consumes the runtime frame and saved "
            "continuation, not caller arithmetic flags: DEC sets the tested SF; "
            "JCXZ tests a popped word. The event path enters EXSA, whose first CMP "
            "kills incoming flags (rtenexit.asm 004e/0068). The error path enters "
            "ERR_RG, which reaches XOR BH,BH before further dispatch (erproc.asm "
            "00ad/00c1). All six GP inputs retained conservatively; BP/SP, segments "
            "and direction remain runtime environment. No preservation, cleanup or "
            "ordinary-return claim. Frontend control flow leaves at RETA."
        ),
    )
    for _name in ("B$ONTA", "B$ETT0", "B$ETT1", "B$ETT2"):
        VARIANTS[(_name, _family)] = replace(
            worst(_name),
            inputs=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
            evidence=(
                "BCL71ENR.LIB/VBDCL10E.LIB evttim.asm: ONTA at 002e/002f "
                "executes OR AX,DX at 003b/003c before every branch or call; "
                "ETT0/1/2 converge on XOR BL,BL at 0023/0024 before EVNT_SET. "
                "Incoming arithmetic flags cannot reach a dependency. Bound inputs "
                "by all six allocatable GP registers; segments, BP/SP and direction "
                "are runtime environment. No transitive preservation, cleanup, "
                "memory or control claim: error paths and indirect dependencies "
                "remain worst-case. See docs/event-entry-blocker.md."
            ),
        )

for _family in ("qb45", "pds71", "vbdos"):
    VARIANTS[("B$FCMD", _family)] = replace(
        worst("B$FCMD"),
        inputs=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
        cleanup=0,
        evidence=(
            "BCOM45.LIB/BCL71ENR.LIB/VBDCL10E.LIB oscmd.asm B$FCMD at 1:0023: "
            "balanced local saves and RETF (0041 in QB/PDS, 004a in VBDOS), no caller arguments. "
            "Its first call is B$CmdCopy at 1:0000, whose XOR BX,BX at 0002 kills incoming "
            "arithmetic flags before any conditional use or further call. Bound inputs by all "
            "six allocatable GP registers; segments, BP/SP and direction are runtime environment. "
            "Heap helper dependencies are incomplete, so memory, clobber and control effects "
            "remain worst-case; no preservation or termination claim."
        ),
    )


def own(name: str) -> Contract:
    """A call into the program's own code, by name.

    BC compiles a SUB as a PUBDEF of the same object and calls it through
    an EXTDEF fixup, so `Module.calls` names it like a runtime routine and
    only the PUBDEF set tells the two apart. A BASIC argument arrives as a
    far pointer on the stack, so nothing arrives in a register -- and that
    is the only thing established here. Everything else stays at the worst
    case: what such a procedure clobbers is as unknown as any other.
    """
    return replace(
        worst(name),
        inputs=frozenset(),
        evidence="a PUBDEF of this same module; BC pushes a far pointer per BASIC argument",
    )


def _entry(family: str) -> None:
    VARIANTS[("B$ENRA", family)] = replace(
        worst("B$ENRA"),
        inputs=frozenset({Reg.CX}),
        cleanup=0,
        evidence=(
            "disassembled from the linked image: `push cx` then `sub sp,cx` before any write, "
            "no unresolved edge from entry to the far jump that returns"
        ),
    )


for _one in ("pds71", "qb45"):
    _entry(_one)

for _name in ("B$SIN4", "B$SIN8", "B$COS4", "B$COS8"):
    VARIANTS[(_name, "vbdos")] = replace(
        worst(_name),
        inputs=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
        cleanup=0,
        evidence=(
            "VBDCL10E.LIB 87btrig.asm SIN4/SIN8 share 005a: ST0 operand, "
            "local BP frame, hardware exits 0093..0096 and 00c3..00c6 restore SP/BP and RETF; "
            "emulator tail B$EMSIN (embtrig.asm 0065) exits 00c8..00cb identically. "
            "COS4/COS8 share 0007 with the same local frame: hardware exits "
            "0040..0043 or 00c3..00c6; emulator tail B$EMCOS (embtrig.asm 0000) "
            "restores SP/BP and RETF at 0061..0064. "
            "All GP inputs are conservatively retained; no preservation, purity, x87 "
            "optimization or error-path guarantee is inferred. Error tails remain unknown."
        ),
    )

VARIANTS[("B$PEOS", "vbdos")] = replace(
    worst("B$PEOS"),
    inputs=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
    evidence=(
        "VBDCL10E.LIB prnval.asm 018f: establishes BP, saves ES/SI, loads FInput "
        "and OR AL,AL at 0197 kills incoming arithmetic flags before any branch/call. "
        "All GP inputs retained; BP/SP, segments and direction are runtime environment. "
        "Terminal input relocates the frame at 019d..01b7; disk/print paths skip it. "
        "Cleanup remains unknown, as do all memory, clobber, control and error effects."
    ),
)

VARIANTS[("B$CHOU", "vbdos")] = replace(
    worst("B$CHOU"),
    inputs=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
    cleanup=2,
    evidence=(
        "VBDCL10E.LIB pr0a.asm 0055: PUSH BP / MOV BP,SP; reads channel at [bp+6], "
        "calls B$ChkFNUM and B$SetFNum, POP BP / RETF 2 at 0061..0064. "
        "All GP inputs and unknown clobber/memory/control/error effects retained; "
        "only the normal-return argument cleanup is established. Library SHA256 "
        "59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301."
    ),
)

for _name, _cleanup, _evidence in (
    ("B$FLEN", 2,
     "farstr/stcore.asm 02b9 reads a far-string descriptor at [bp+6]; OR AX,AX "
     "at 02c1 kills incoming arithmetic flags before any branch/dependency. "
     "Empty and nonempty paths join POP BP / RETF 2 at 02f6..02f9. "
     "Temporary strings call FreeDataPpv with two words (RETF 4 at 01da), "
     "which calls FreeHandle (RET at 010c). Freeing may mutate aliased heap "
     "state; no memory or register preservation is claimed."),
    ("B$RDIM", None,
     "erase.asm 0000 establishes BP and reads descriptor [bp+6]; OR BL,BL "
     "at 0009 kills incoming flags before branches or dependencies. All GP "
     "inputs retained. DIM_COMMON consumes rank-dependent stack arguments, "
     "so cleanup stays unknown unless separately proven at the call site."),
    ("B$FEOF", 2,
     "dvstmt.asm 0073 reads file word [bp+6]; OR BX,BX kills incoming flags. "
     "File and DOS console paths join POP BP / RETF 2 at 0097..009a; "
     "invalid console mode tails ERR_IFN. DOS/device/error effects remain unknown."),
    ("B$CLOS", None,
     "dvcore.asm 01b4 reads a stack count and file words; JCXZ selects CLOSF "
     "or LocateFDB. CLOSF's CMP at 019f and LocateFDB's XOR SI,SI at 00e6 "
     "kill incoming flags before dependencies. Return restores SP from the advanced "
     "argument cursor at 01e3, so cleanup stays unknown, not zero."),
    ("B$ERAS", 2,
     "erase.asm 0020 reads descriptor [bp+6]; empty arrays go directly to epilogue; "
     "other paths test descriptor flags before dependencies. All normal paths join "
     "POP DI/SI/BP / RETF 2 at 00bf..00c4. Heap, alias and error effects remain unknown."),
    ("B$OPEN", 8,
     "dkutil.asm 00c0..00f4 reads four stack words, calls DOS3CHECK and OPENIT, "
     "and restores BP then RETF 8 at 00f1. Other branches tail ERR_AFE or ERR_IFN; "
     "device, allocation and error effects are not established."),
    ("B$DSKI", 2,
     "inpdsk.asm 0016..0060 reads [bp+6], calls ChkFNUM/LocateFDB/EnsureFI, "
     "sets input state, restores SI/BP and RETF 2 at 005e. Other branches tail "
     "ERR_IFN, ERR_RPE or ERR_BFM; no device or error-path guarantees."),
    ("B$FREF", 0,
     "dvstmt.asm 0031..0051 walks B$NextFDB, restores SI/BP and RETF. "
     "NextFDB saves AX/BX/CX/DX and calls PpvWalkHeap with two words; "
     "lwalk.asm PpvWalkHeap returns RETF 4 at 0039. No caller arguments."),
    ("B$LDFS", 6,
     "string.asm 0030..005b loads [bp+6/+8/+0a], optionally calls B$AlcTmpSH "
     "and copies bytes; both zero-length and copying paths restore DS/DI/SI/BP "
     "then RETF 6. Allocator and error dependencies remain unproved."),
):
    VARIANTS[(_name, "vbdos")] = replace(
        worst(_name),
        inputs=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
        cleanup=_cleanup,
        evidence=("VBDCL10E.LIB: " + _evidence +
                  " All GP inputs and unknown effects retained; cleanup only where stated. "
                  "SHA256 59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301."),
    )

for _name, _cleanup in (("B$PCR4", 4), ("B$PSR4", 4),
                        ("B$PCR8", 8), ("B$PSR8", 8), ("B$PER8", 8)):
    VARIANTS[(_name, "vbdos")] = replace(
        worst(_name),
        inputs=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
        cleanup=_cleanup,
        evidence=(
            "rt/prnvalfp.asm: scalar argument pushed by value; entry sets "
            "AL=VT_R4 (4) or VT_R8 (8), AH=terminator, then tails B$PRINT. "
            "VBDCL10E.LIB prnvalfp.asm entries 0000/0006/0012/0018/001e "
            "match; prnval.asm 003d saves the type and 0087 reloads it. "
            "Normal exits select RETF 4 at 009a or RETF 8 at 0094. "
            "Source PRINTX documents the same argument-width cleanup. "
            "Device dispatch and error effects remain unknown; all GP inputs retained. "
            "Library SHA256 59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301."
        ),
    )

VARIANTS[("B$EXSA", "vbdos")] = replace(
    worst("B$EXSA"),
    inputs=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
    cleanup=0,
    evidence=(
        "VBDCL10E.LIB rtenexit.asm, seg 1:0x68: CMP [bp-12h],0 kills incoming arithmetic flags. "
        "Bound inputs by all six allocatable GP registers, not by incomplete helper summaries. "
        "The normal exit saves/restores DX:AX, restores the frame at 0x8e..0x97, then jumps "
        "through the return address popped at 0x6e/0x72; it removes no caller arguments. "
        "BP/SP and segments are the fixed runtime environment. Indirect/error/helper effects "
        "remain unknown: no clobber, memory or control guarantee is relaxed."
    ),
)

# B$EXSA under PDS 7.1, bounded from the linked image at 0x1d6c. Its
# returning path -- `pop word [422h]`/`[424h]`, `lea sp,[bp-6]`, four pops,
# `jmp far [422h]` -- reads no register and removes no caller argument, so
# cleanup is 0. Its other path is error dispatch, and there the survivors
# are what is declared here rather than what is read: bx is fully written
# (`mov bl,13h` at 0x19da, `xor bh,bh` at 0x1a63) and ax by the entry
# table, but cx survives the arm that skips `mov cx,[41Ch]`, and dx, si and
# di are never written on any path before `jmp cx` at 0x1a79 or the `retf`
# at 0x1b10. A superset, which costs a copy where it is wrong and cannot
# be unsound; the control semantics past that indirect jump stay unknown,
# which is what the conservative fields still say.
VARIANTS[("B$EXSA", "pds71")] = replace(
    worst("B$EXSA"),
    inputs=frozenset({Reg.AX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
    cleanup=0,
    evidence=(
        "disassembled from the linked image at 0x1d6c: the returning path reads no register "
        "and removes no argument; on the error path ax and bx are written before every "
        "terminal, and cx, dx, si and di are not. The normal return continuation "
        "exports dx:ax to the BASIC caller, so both halves are live at this boundary."
    ),
)

# QuickBASIC 4.5's, at 0x20f2: no register read on any reachable path, one
# exit, no unresolved edge.
VARIANTS[("B$EXSA", "qb45")] = replace(
    worst("B$EXSA"),
    inputs=frozenset({Reg.AX, Reg.DX}),
    cleanup=0,
    evidence=(
        "disassembled from the linked image at 0x20f2: no register read, one exit; "
        "the normal return continuation exports dx:ax to the BASIC caller"
    ),
)


def for_module(found, *, external: dict[str, Contract] | None = None) -> "dict[int, Contract]":
    """The per-site map for a whole module, from the object itself.

    One place, because the raise and the lowering must be handed the same
    answer: built twice from different arguments they can differ, and
    `test_the_allocator_settles_on_every_program` did exactly that -- the
    raise establishing PDS's B$ENRA while the lowering saw the
    conservative contract, leaving a pin naming a value the body no longer
    held.
    """
    from qbopt.objectfile import module

    family = module.family(found.records)
    contracts = per_call(found.calls, family, module.defines(found.records, found.seg))
    from qbopt.abi import events
    contracts.update(events.contracts(found))
    if family == "vbdos":
        _zero_entry_sites(found, contracts)
        _redim_sites(found, contracts)
    for name, routine in (external or {}).items():
        if routine.name != name:
            raise ValueError(f"external contract name mismatch: {name!r} != {routine.name!r}")
        for at, called in found.calls.items():
            if called == name:
                contracts[at] = routine
    return contracts


def _redim_sites(found: "Module", contracts: dict[int, Contract]) -> None:
    """B$ExitDim removes three header words and two bound words per dimension."""
    from iced_x86 import Code
    from qbopt.frontend import blocks

    if "B$RDIM" not in found.calls.values():
        return
    mapped = blocks.code_map(found)
    if isinstance(mapped, str):
        return
    for block in blocks.partition(found, mapped):
        for rank, descriptor, call in zip(block.insns, block.insns[1:], block.insns[2:]):
            if found.calls.get(call.at) != "B$RDIM" or rank.end != descriptor.at or descriptor.end != call.at:
                continue
            if rank.insn.code not in {Code.PUSH_IMM16, Code.PUSHW_IMM8}:
                continue
            if descriptor.insn.code not in {Code.PUSH_IMM16, Code.PUSHW_IMM8, Code.PUSH_R16, Code.PUSH_RM16}:
                continue
            if any(rank.at <= field < rank.end for field in found.fixup_at):
                continue
            dimensions = rank.insn.immediate(0) & 255
            contracts[call.at] = replace(
                worst("B$RDIM"),
                inputs=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI}),
                cleanup=6 + 4 * dimensions,
                evidence=(
                    "VBDCL10E.LIB erase.asm RDIM tails dynamic.asm DIM_COMMON. "
                    "ExitDim 010a reads [bp+8], clears CH, doubles twice and adds 6; "
                    "011d..0129 pops the return address, adds that count to SP and jumps back. "
                    "Rank is an unrelocated immediate word push immediately before the descriptor "
                    "and call in one basic block. All GP inputs and unknown effects retained."
                ),
            )


def _zero_entry_sites(found: "Module", contracts: dict[int, Contract]) -> None:
    """VBDOS's zero-BX entry bypasses its unresolved helper call."""
    from iced_x86 import Code
    from iced_x86 import Register

    from qbopt.frontend import blocks

    if "B$ENRA" not in found.calls.values():
        return
    mapped = blocks.code_map(found)
    if isinstance(mapped, str):
        return
    for block in blocks.partition(found, mapped):
        for previous, call in zip(block.insns, block.insns[1:], strict=False):
            if found.calls.get(call.at) != "B$ENRA" or previous.end != call.at:
                continue
            insn = previous.insn
            if insn.code != Code.MOV_R16_IMM16 or insn.op0_register != Register.BX or insn.immediate16 != 0:
                continue
            if any(previous.at <= field < previous.end for field in found.fixup_at):
                continue
            contracts[call.at] = replace(
                worst("B$ENRA"),
                inputs=frozenset({Reg.BX, Reg.CX}),
                cleanup=0,
                evidence=(
                    "VBDCL10E.LIB rtenexit.asm B$ENRA 0x17..0x55: CX sizes the frame; "
                    "BX=0 at 0x4b bypasses the helper at 0x5b. A same-block immediate MOV BX,0 "
                    "immediately precedes this call. All other effects remain worst-case."
                ),
            )


def per_call(
    calls: "dict[int, str]", family: str = "", defined: "frozenset[str]" = frozenset()
) -> "dict[int, Contract]":
    """One contract per call site, chosen once for the whole module.

    A side map rather than a field on the body: a contract is a fact about
    the machine, and the raise and the lowering both need the same answer
    for the same call. Passing it keeps them from looking one up
    separately and disagreeing -- which is what a per-family contract
    makes possible, since B$ENRA reads bx under one runtime and not
    another.

    `family` is `module.Family`'s value, as a string so this stays below
    module.py. It selects a variant where one is established and changes
    nothing otherwise -- a caller with no family in hand gets the
    conservative contract, which is the honest answer for an object whose
    toolchain nobody read.
    """
    return {
        at: (own(name) if name in defined else VARIANTS.get((name, family)) or contract(name))
        for at, name in calls.items()
    }


def established_inputs(routine: Contract) -> bool:
    """Whether this routine's inputs are established at all.

    `inputs` is None where nothing is known and an empty set where the
    routine is known to read no register. The two are not the same fact
    and reading them as one is how a call to a routine whose code is not
    in the tree came out with no requirements at all -- which says it
    reads nothing, the one thing known to be false about it.
    """
    return routine.inputs is not None


def slots(routine: Contract) -> tuple[Reg, ...]:
    """Which registers this routine reads its arguments in, in slot order.

    Empty where nothing is established: a routine whose code is not in the
    tree declares no inputs, and the conservative read set the raise builds
    for it is a liveness dependency rather than an argument list.
    """
    if not routine.inputs:
        return ()
    return tuple(one for one in SLOTS if one in routine.inputs)


def writes_caller_memory(routine: Contract) -> bool:
    return routine.writes >= Memory.STRINGS


def barrier(routine: Contract) -> bool:
    """Whether a pass keeping caller values in registers must refuse a body
    holding this call outright, rather than reasoning about what it touches."""
    return routine.enters_user_code or routine.error_handling or not routine.established


# helpi4.asm is the model the four shipped contracts in calls.py and stack.py
# were read off, and nothing below contradicts them. cProc's parm declarations
# give the cleanup directly: cEnd emits `ret <parameter bytes>` under the PL/M
# convention (inc/cmacros.inc), and parmD is 4 bytes, parmW and parmSD 2
# (inc/string.inc makes parmSD an alias for parmW).
# All five print entry points in the corpus set ax to a [terminator|value type]
# pair and fall into B$PRINT, which pops its own arguments in a hand-written
# epilogue keyed on that type byte (rt/prnval.asm PRINTX): one word for I2 and
# SD, two for I4. That epilogue also re-pushes the far return address and RETs,
# so control does come back to the byte after the call.
_PRINT_CLOBBERS = EVERY - {Reg.BP, Reg.SI, Reg.SP}


def numeric_print_argument(name: str) -> int | None:
    """Bytes consumed by value by prnval.asm's PRINTX path, not a descriptor pointer."""
    if name not in {"B$PEI2", "B$PSI2", "B$PEI4", "B$PSI4", "B$PER4"}:
        return None
    routine = contract(name)
    return routine.cleanup if routine.established else None


def numeric_stack_arguments(name: str) -> int | None:
    """Known scalar stack arguments, whose bits are values rather than caller pointers."""
    if name in {"B$MUI4", "B$DVI4", "B$RMI4", "B$CPI4"}:
        routine = contract(name)
        return 8 if routine.established and routine.cleanup == 8 else None
    return numeric_print_argument(name)

_PRINT_EVIDENCE = (
    "rt/prnval.asm: the entry point is `MOV AX,<term> SHL 8 + <type>` then `JMP SHORT B$PRINT` (B$PESD falls "
    "straight in). `cProc B$PRINT,<PUBLIC,FAR>,<SI>` saves si and the epilogue restores si and bp before "
    "returning, so those two survive; the rest does not, because the numeric path calls B$FOUTBX and the file "
    "comment there says it `changes all registers except BP` -- flatly contradicting ifout.asm's "
    '"Per convention. (DS, ES, SI, DI, BP preserved.)" for the same routine, and the conservative reading wins. '
    "PRINTX pops one parameter word, then a second unless `TEST AL,VT_SD` is non-zero, which it is for VT_I2=2 "
    "and VT_SD=3 and is not for VT_I4=14h (inc/rtps.inc). Memory is the worst case and read as such: print using "
    "goes through the [b$PUSG] vector into prtu.asm, which calls B$SASS, and output goes through the [VTYP], "
    "[VWCH] and [b$pFLUSH] vectors whose targets depend on the open device. Any string allocation on those paths "
    "reaches B$STALC, whose step 4 calls B$STCPCT -- and rt/nhstutil.asm says of it `The string descriptors "
    "referenced by the string header are adjusted to reflect their movement`, writing [BX+2] of every live "
    "descriptor including a module-level string variable's. Header: `Exceptions: bad file mode; I/O error or "
    "Disk full error when flush the buffer if a EOL encountered`."
)


def _print(name: str, cleanup: int) -> Contract:
    return Contract(
        name=name,
        # The stub sets ax itself before jumping to B$PRINT, and BC pushes
        # the value: nothing here is a register the caller has to have set.
        inputs=frozenset(),
        cleanup=cleanup,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.OWN,
        reads=Memory.OWN,
        clobbers=_PRINT_CLOBBERS,
        established=True,
        evidence=_PRINT_EVIDENCE,
    )


# Everything here reaches user BASIC, or leaves without coming back, or both.
# Their details are recorded for the record; barrier() is the field that
# matters, and it is true for every one of them.
# The three whose own implementation is not in the local tree. B$ENRA and
# B$EXSA are named only by the include files, and B$OGTA only by ulib.inc and
# rtmint.inc -- extent.py's docstring already says so and this does not repeat
# the reasoning. What is established about them is established from the frame
# layout and from measurements of BC's output, not from read code, so
# everything except the fields named in each citation stays at the worst case.
# The x87 helpers, all six of them one module -- 87bhelp.asm -- and byte for
# byte the same object in QuickBASIC 4.5's BCOM45.LIB, PDS 7.1's BCL71ENR.LIB
# and VBDOS 1.0's VBDCL10E.LIB. They are the one group here established from
# a disassembly rather than from source: runtime/inc/rtmint.inc declares them
# and nothing in the 148-file runtime tree defines them, because the math
# library is not in the source drop. tools/libdump.py is what read them out.
#
# Every one of them keeps its own frame (push bp / mov bp,sp ... mov sp,bp /
# pop bp / retf), takes no argument on the 8086 stack -- the operands are in
# registers or already on the x87 stack -- and touches no memory but its own
# scratch below sp. So cleanup is 0 and writes is NONE throughout, and what
# differs between them is only which registers come back changed.
def _read(name: str) -> Contract:
    """One of READ's per-type entries.

    rt/read.asm: `B$RD<type> only sets the type, [b$VTYP], and then jump to
    a common routine, CommRead`. Entry is `pDest = far pointer to the
    destination for the data`, so four bytes come off; `cbDest` is a fifth
    parameter for SD and FS only, and none of those is here.

    It writes through pDest and reads the DATA area, and CommRead reaches
    B$ReadVal through the [b$GetOneVal] vector -- so Memory.ANY on both
    sides rather than the destination alone. `Uses: per convention` names
    nothing preserved. It raises: out of DATA, syntax error, overflow.
    """
    return Contract(
        name=name,
        # pDest is a parameter, so nothing arrives in a register.
        inputs=frozenset(),
        cleanup=4,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.OWN,
        reads=Memory.OWN,
        clobbers=EVERY,
        established=True,
        evidence=(
            "rt/read.asm, the header above B$RDI2: `pDest = far pointer to the destination for the "
            "data`, `cbDest = for SD and FS only`; `B$RD<type> only sets the type, [b$VTYP], and then "
            "jump to a common routine, CommRead`. CommRead reaches B$ReadVal through the "
            "[b$GetOneVal] vector, so what it writes is not bounded by pDest. `Uses: per convention` "
            "names no preserved register. Exceptions: out of DATA, syntax error, overflow."
        ),
    )


# Where the rows live. Data, not code: every field is a claim about the
# runtime, and a claim wants an audit trail more than it wants a Python
# literal. `tools/runtime_writes.py` regenerates the measured columns from
# a linked image, so re-measuring is a diff against this file rather than a
# rewrite of one.
TABLE = Path(__file__).with_name("runtime.toml")


def _contracts(path: Path | None = None) -> dict[str, Contract]:
    """One entry per runtime name the corpus calls, read from the table.

    B$OGTA's control kind comes from blocks.py rather than being restated
    in the table, which is why this is not simply the rows handed back.
    """
    rows = tomllib.loads((path or TABLE).read_text())
    out = {}
    for name, row in rows.items():
        one = Contract(
            name=name,
            cleanup=None if row["cleanup"] < 0 else row["cleanup"],
            control=Control(row["control"]),
            enters_user_code=row["enters_user_code"],
            raises_error=row["raises_error"],
            error_handling=row["error_handling"],
            writes=Memory[row["writes"]],
            reads=Memory[row["reads"]],
            clobbers=frozenset(Reg(one) for one in row["clobbers"]),
            established=row["established"],
            evidence=row["evidence"].strip(),
            documented=frozenset(Reg(x) for x in row["documented"]) if "documented" in row else None,
            inputs=frozenset(Reg(x) for x in row["inputs"]) if "inputs" in row else None,
        )
        out[name] = replace(one, control=Control.INLINE_TABLE) if name in INLINE_TABLE else one
    return out


CONTRACTS = _contracts()
CONTRACTS["B$HARY"] = VARIANTS[("B$HARY", "qb45")]
CONTRACTS["B$LINA"] = VARIANTS[("B$LINA", "qb45")]

for _family, _library, _offset in (
    ("pds71", "BCL71ENR.LIB", "0103"),
    ("vbdos", "VBDCL10E.LIB", "0127"),
):
    VARIANTS[("B$EVK1", _family)] = replace(
        CONTRACTS["B$EVCK"],
        name="B$EVK1",
        evidence=(
            f"{_library} evtcore.asm PUBDEF: B$EVK1 and B$EVCK both name "
            f"segment 1 offset {_offset}; exact entry aliases in this runtime family. "
            "Library hashes and scope: docs/event-entry-blocker.md. "
            + CONTRACTS["B$EVCK"].evidence
        ),
    )


# Routines that hand control back to the program. What they write is what
# the code they call writes, which is anything -- GCC's modref gives up on
# an indirect call for the same reason. B$CENP ends the program, B$EVCK
# polls for an event and may run an event GOSUB, B$OEGA and B$RESN are the
# ON ERROR machinery, and B$FCMD is not established at all.
ENTERS_USER_CODE = frozenset({"B$CENP", "B$EVCK", "B$FCMD", "B$OEGA", "B$RESN"})


def contract(name: str | None) -> Contract:
    """What this call can do. A name with no entry -- a user SUB or FUNCTION, a
    runtime routine nothing in the corpus has called yet, an indirect call with
    no name at all -- gets the worst case rather than an absence."""
    if name is None:
        return worst("")
    return CONTRACTS.get(name, worst(name))
