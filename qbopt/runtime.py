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

from enum import IntEnum
from enum import StrEnum
from dataclasses import field
from dataclasses import replace
from dataclasses import dataclass

from qbopt.blocks import INLINE_TABLE


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
    ANY = 3


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
_HELPERS = (
    Contract(
        name="B$CPI4",
        cleanup=8,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.NONE,
        reads=Memory.ARGUMENTS,
        clobbers=frozenset({Reg.FLAGS}),
        documented=frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX}),
        established=True,
        evidence=(
            "rt/helpi4.asm: `cProc B$CPI4,<FAR,PUBLIC>,<AX>` with parmD op1, parmD op2. The body reads both "
            "arguments off the frame and ends with lahf/and/shr/shl/or/sahf -- it names cx, dx and bx nowhere, "
            "and the <AX> save list restores ax, where B$MUI4/B$DVI4/B$RMI4 declare an empty one. Its own "
            '"Uses: ax,cx,dx,bx" overstates that, and its "Exceptions: hardware divide overflow" is copied from '
            "the divide above it: there is no division in the routine. Flags are the return value."
        ),
    ),
    Contract(
        name="B$MUI4",
        cleanup=8,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.NONE,
        reads=Memory.ARGUMENTS,
        clobbers=PER_CONVENTION,
        established=True,
        evidence=(
            "rt/helpi4.asm: `B$MUI4(I4 op1,I4 op2)`, `Uses: ax,cx,dx,bx`, `Exceptions: none`, and a body that is "
            "`jmp __aFlmul` with no frame. __aFlmul is the C library's own long multiply and is NOT in the local "
            "tree, so the register set is the header's word rather than read code; the cleanup is the C helper's, "
            "and BC's measured call sites (calls.py, stack.py) say it happens. AGENTS.md measured the overflow: "
            "305419896 * 252645135 wraps and raises nothing."
        ),
    ),
    Contract(
        name="B$DVI4",
        cleanup=8,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.NONE,
        reads=Memory.ARGUMENTS,
        clobbers=PER_CONVENTION,
        established=True,
        evidence=(
            "rt/helpi4.asm: `jmp __aFldiv`, `Uses: ax,cx,dx,bx`, `Exceptions: hardware divide overflow`. __aFldiv "
            "is not in the local tree. AGENTS.md measured what the header calls a hardware trap: `x \\ 0` raises "
            "BASIC error 11, and -2147483648 \\ -1 raises nothing at all and returns. calls.py absorbs this call "
            "as a bare idiv and drops error 11 deliberately, which is the one place a rewritten program behaves "
            "worse than BC's."
        ),
    ),
    Contract(
        name="B$RMI4",
        cleanup=8,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.NONE,
        reads=Memory.ARGUMENTS,
        clobbers=PER_CONVENTION,
        established=True,
        evidence=(
            "rt/helpi4.asm: `jmp __aFlrem`, otherwise identical to B$DVI4, and __aFlrem is not in the tree either."
        ),
    ),
)


# All five print entry points in the corpus set ax to a [terminator|value type]
# pair and fall into B$PRINT, which pops its own arguments in a hand-written
# epilogue keyed on that type byte (rt/prnval.asm PRINTX): one word for I2 and
# SD, two for I4. That epilogue also re-pushes the far return address and RETs,
# so control does come back to the byte after the call.
_PRINT_CLOBBERS = EVERY - {Reg.BP, Reg.SI, Reg.SP}

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
        cleanup=cleanup,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.ANY,
        reads=Memory.ANY,
        clobbers=_PRINT_CLOBBERS,
        established=True,
        evidence=_PRINT_EVIDENCE,
    )


_PRINTING = (
    _print("B$PSI2", 2),
    _print("B$PEI2", 2),
    _print("B$PEI4", 4),
    _print("B$PSSD", 2),
    _print("B$PESD", 2),
)


_STRINGS = (
    Contract(
        name="B$SASS",
        cleanup=4,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.STRINGS,
        reads=Memory.STRINGS,
        clobbers=PER_CONVENTION,
        established=True,
        evidence=(
            "rt/stcore.asm: `cProc B$SASS,<FAR,PUBLIC>` with parmW psdSource, parmW psdDst, `Registers: per "
            "convention`. It writes the caller's destination descriptor outright -- `MOV [BX],CX` and "
            "`MOV [BX+2],AX` with BX=psdDst -- plus the backpointer at [BX-2] in string space, and the NOTEMP "
            "path calls B$STALCTMPSUB, which reaches B$STALC and so B$STCPCT (rt/nhstutil.asm), moving every "
            "other live string and rewriting its descriptor. B$STALC's failure path is `JMP B$ERR_OS`."
        ),
    ),
    Contract(
        name="B$SCAT",
        cleanup=4,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.STRINGS,
        reads=Memory.STRINGS,
        clobbers=PER_CONVENTION,
        established=True,
        evidence=(
            "rt/stcore.asm: `cProc B$SCAT,<FAR,PUBLIC>,<ES,DI,SI>` with parmW psd1, parmW psd2, so es, di and si "
            "come back despite the routine setting ES=DS internally. It calls B$STALCTMP for the result, which "
            "reaches B$STCPCT and rewrites live descriptors (rt/nhstutil.asm), and MOVSTR ends `JMP B$STDALCTMP` "
            "on each source, freeing it if it was a temp. Length overflow is `JO ERRFC` -> `JMP B$ERR_FC`."
        ),
    ),
    Contract(
        name="B$STDL",
        cleanup=2,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.STRINGS,
        reads=Memory.STRINGS,
        clobbers=PER_CONVENTION,
        established=True,
        evidence=(
            "rt/string.asm: `cProc B$STDL,<PUBLIC,FAR>` with parmW pSd, `Uses: Per convention`, "
            "`Exceptions: none`. Its last instruction is `MOV WORD PTR [BX],0` on the descriptor it was handed -- "
            "a write straight into a caller variable. It reaches only B$STDALC, which goes to B$STADJ and never "
            "calls B$STCPCT (rt/nhstutil.asm), so unlike an allocation it moves no other string."
        ),
    ),
    Contract(
        name="B$STI2",
        cleanup=2,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.STRINGS,
        reads=Memory.ARGUMENTS,
        clobbers=EVERY - {Reg.BP, Reg.SP},
        documented=PER_CONVENTION,
        established=True,
        evidence=(
            "rt/string.asm: `cProc B$STI2,<PUBLIC,FAR>` with ParmW I2Arg, `Uses: Per convention`, "
            "`Exceptions: Out of memory`. It falls into B$STR_COMMON, which sets b$VTYP, calls B$FOUTBX and then "
            "B$STALCTMPCPY -> B$STALCTMP -> B$STALC -> B$STCPCT, so it can move every live string and rewrite "
            "its descriptor. The header understates the clobber set in the direction B$CPI4's overstates it: "
            "rt/prnval.asm says B$FOUTBX `changes all registers except BP`, so si, di, ds and es are not the "
            '"per convention" survivors ifout.asm claims.'
        ),
    ),
    Contract(
        name="B$LTRM",
        cleanup=2,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.STRINGS,
        reads=Memory.STRINGS,
        clobbers=PER_CONVENTION,
        established=True,
        evidence=(
            "rt/strfcn.asm: B$LTRM is `MOV AH,TR_LEFT` then a two-byte skip into `cProc TRIM,FAR,<DI,ES>` with "
            "parmW psd, `Uses: Per convention`, so di and es come back and the cleanup is TRIM's 2 bytes. It "
            "scans the caller's string data and calls B$STALCTMPSUB for the result, reaching B$STCPCT. The "
            "right-trim path sets the direction flag but every exit passes the following CLD, matching "
            "ifout.asm's `PSW.D clear`."
        ),
    ),
    Contract(
        name="B$FVAL",
        cleanup=2,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.ANY,
        reads=Memory.STRINGS,
        clobbers=EVERY - {Reg.BP, Reg.SP, Reg.SI},
        established=True,
        evidence=(
            "rt/fin.asm: `cProc B$FVAL,<FAR,PUBLIC>,<SI>` with parmSD sdNum (parmW, per inc/string.inc), so si "
            "survives whatever the scanner does. It calls B$STPUTZ to zero-terminate the caller's string data in "
            "place, sets b$VTYP, runs B$FIN and returns the address of B$DAC; B$FIN is the mathpack's input "
            "scanner and floating point, not modelled here, and its errors are B$ERR_OV and B$ERR_TM. Memory is "
            "the worst case because B$FIN was not read."
        ),
    ),
)


_SIMPLE = (
    Contract(
        name="B$DSEG",
        cleanup=2,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.NONE,
        reads=Memory.ARGUMENTS,
        clobbers=frozenset({Reg.AX}),
        documented=frozenset(),
        established=True,
        evidence=(
            "rt/rtinit.asm: `cProc B$DSEG,<PUBLIC,FAR>` with parmW newseg, whole body `MOV AX,newseg` / "
            "`MOV [b$seg],AX`, header `Modifies: NONE` -- which is wrong by one register. With the cmacros "
            "prologue and epilogue that is push bp / mov bp,sp / two movs / mov sp,bp / pop bp / ret 2, and not "
            "one of them writes a flag. b$seg is the runtime's own DGROUP word that PEEK and POKE read later, "
            "not anything the caller can name."
        ),
    ),
    Contract(
        name="B$FERR",
        cleanup=0,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=True,
        writes=Memory.NONE,
        reads=Memory.NONE,
        clobbers=frozenset({Reg.AX}),
        established=True,
        evidence=(
            "rt/error.asm: `cProc B$FERR,<FAR,PUBLIC>` with no parameters and the single instruction "
            "`MOV AX,[b$errnum]` -- the ERR function, and nothing more. It cannot itself reach a handler, "
            "contrary to the rest of its file; what it does prove is that the module has ON ERROR, which is what "
            "error_handling records and why barrier() still refuses a body containing it."
        ),
    ),
    Contract(
        name="B$?EVT",
        cleanup=0,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.ANY,
        reads=Memory.NONE,
        clobbers=EVERY - {Reg.BP, Reg.SP, Reg.SI, Reg.DI, Reg.ES},
        established=True,
        evidence=(
            "rt/gwaevt.asm: `cProc B$?EVT,<FAR,PUBLIC>,<ES,SI,DI>`, no parameters. It clears b$TRPTBL to "
            "flag-zero/address--1 entries, zeroes b$TRAP_SEM, inits the b$TRAP_QUE descriptor through B$INITQ "
            "and calls [b$pInitKeys1] and [b$pInitKeys2] -- all runtime state, no user variable in sight, and it "
            "installs handlers rather than running one. Memory stays the worst case because those three callees "
            "were not followed. Emitted only under /V or /W, which the optimiser refuses anyway."
        ),
    ),
)


# Everything here reaches user BASIC, or leaves without coming back, or both.
# Their details are recorded for the record; barrier() is the field that
# matters, and it is true for every one of them.
_CONTROL = (
    Contract(
        name="B$EVCK",
        cleanup=0,
        control=Control.UNKNOWN,
        enters_user_code=True,
        raises_error=False,
        error_handling=False,
        writes=Memory.ANY,
        reads=Memory.ANY,
        clobbers=EVERY,
        established=True,
        evidence=(
            "rt/gwaevt.asm: `cProc B$EVCK,<PUBLIC,FAR>`, no parameters, `Exit: GOSUBs to event handler, if so "
            "indicated`. On a pending trap it dequeues the request, calls B$FRAMESETUP, bumps FR_GOSUB and ends "
            "`jmp dword ptr[BX+1]` into the user's ON TIMER/ON KEY handler (or B$IEvHandler, which `DOESN'T "
            "RETURN`). So it can assign any module-level variable the handler names, and control leaves through "
            "the event-GOSUB machinery rather than off its own RET."
        ),
    ),
    Contract(
        name="B$OEGA",
        cleanup=4,
        control=Control.UNKNOWN,
        enters_user_code=True,
        raises_error=False,
        error_handling=True,
        writes=Memory.ANY,
        reads=Memory.ANY,
        clobbers=EVERY,
        established=True,
        evidence=(
            "rt/error.asm: `cProc B$OEGA,<FAR,PUBLIC>` with parmD erradr -- the ON ERROR GOTO statement handler. "
            "It sets OFD_ONERROR in the module's own data, and its header says `erradr == 0 and an error is in "
            "progress, will jump to B$SERR instead of returning`: OEGA_10 does `MOV BP,[BP]` / `ADD SP,8` and "
            "`JMP B$SERR ;Process error (Never Returns)`, which is the path into the user's handler."
        ),
    ),
    Contract(
        name="B$RESN",
        cleanup=0,
        control=Control.NEVER,
        enters_user_code=True,
        raises_error=False,
        error_handling=True,
        writes=Memory.ANY,
        reads=Memory.ANY,
        clobbers=EVERY,
        established=True,
        evidence=(
            "rt/error.asm: `cProc B$RESN,<FAR,PUBLIC,FORCEFRAME>,SI`, no parameters -- RESUME NEXT. It searches "
            "the module's statement-address table for the entry after b$erradr and leaves through RESRET to "
            "resume there, or `JMP B$CEND` when it runs off the bottom. It never comes back to the byte after "
            "the call. A /X construct, which the optimiser does not support."
        ),
    ),
    Contract(
        name="B$CENP",
        cleanup=0,
        control=Control.NEVER,
        enters_user_code=True,
        raises_error=False,
        error_handling=False,
        writes=Memory.ANY,
        reads=Memory.ANY,
        clobbers=EVERY,
        established=True,
        evidence=(
            "rt/rtterm.asm: `cProc B$CENP,<PUBLIC,FAR,FORCEFRAME>`, no parameters, `Exceptions: Does not "
            "return.` -- the default end procedure, and the last call in any BASIC module. With an ON ERROR in "
            "progress it is `JMP FAR PTR B$ERR_NR`, handing a No RESUME error to the user's handler; otherwise "
            "it falls into termination. extent.py measured what follows one: in divmod-v-g3.obj the /X RESUME "
            "map begins exactly where its `call far B$CENP` ends, so the bytes after the call are data."
        ),
    ),
    Contract(
        name="B$CEND",
        cleanup=0,
        control=Control.NEVER,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.ANY,
        reads=Memory.ANY,
        clobbers=EVERY,
        established=True,
        evidence=(
            "rt/rtterm.asm: `cProc B$CEND,<PUBLIC,FAR,FORCEFRAME>`, no parameters, `Exceptions: Does not "
            "return.` -- END and SYSTEM. It zeroes b$errnum and branches to the END executor, closing files and "
            "terminating the runtime."
        ),
    ),
)


# The three whose own implementation is not in the local tree. B$ENRA and
# B$EXSA are named only by the include files, and B$OGTA only by ulib.inc and
# rtmint.inc -- extent.py's docstring already says so and this does not repeat
# the reasoning. What is established about them is established from the frame
# layout and from measurements of BC's output, not from read code, so
# everything except the fields named in each citation stays at the worst case.
_FRAMES = (
    Contract(
        name="B$ENRA",
        cleanup=0,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.ANY,
        reads=Memory.ANY,
        clobbers=EVERY,
        established=True,
        evidence=(
            "inc/stack.inc: `The basic frame is set up on entry to the main program, by B$ENSA/B$ENRA SUB and "
            "FUNCTION entry routines`, and it names what lands there -- the previous BASIC frame at FR_BFRAME, "
            "si and di at FR_SI/FR_DI `Preserved for C compatability`, the local byte count at FR_CLOCALS and a "
            "zeroed GOSUB count at FR_GOSUB. inc/rtmint.inc and inc/ulib.inc list it as an entry family with "
            "B$ENSA/B$ENRD/B$ENSD/B$ENFA; the code is in none of them. AGENTS.md measured the call site: the "
            "frame size arrives in cx, so nothing is on the stack to clean up. It moves both sp and bp, which is "
            "why it is in clobbers and why nothing may track stack depth across it."
        ),
    ),
    Contract(
        name="B$EXSA",
        cleanup=0,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.ANY,
        reads=Memory.ANY,
        clobbers=EVERY,
        established=True,
        evidence=(
            "inc/stack.inc: si and di are `restored by B$EXSA`, and [b$curframe] is saved at FR_BFRAME `such "
            "that B$EXSA can restore it on exit`. inc/ulib.inc pairs it with B$EXFA as the exit family and "
            "rt/error.asm calls it to `clear frame state info`. The code is not in the tree. AGENTS.md measured "
            "the call site: a far call with no arguments, immediately before the procedure's own `retf n`. Like "
            "B$ENRA it moves sp and bp."
        ),
    ),
    Contract(
        name="B$OGTA",
        cleanup=None,
        control=Control.UNKNOWN,
        enters_user_code=False,
        raises_error=True,
        error_handling=False,
        writes=Memory.ANY,
        reads=Memory.ANY,
        clobbers=EVERY,
        established=True,
        evidence=(
            "Named by inc/ulib.inc and inc/rtmint.inc; the code is not in the tree. What is established is "
            "measured, not read: blocks.py found that ON GOTO compiles to a call to it followed by inline data "
            "-- a count byte and that many offset16 words, each a fixup into this segment -- which it reads via "
            "its own return address. blocks.INLINE_TABLE is where that lives and where the control kind below "
            "comes from, so the two cannot drift."
        ),
    ),
    Contract(
        name="B$FCMD",
        cleanup=None,
        control=Control.UNKNOWN,
        enters_user_code=True,
        raises_error=True,
        error_handling=False,
        writes=Memory.ANY,
        reads=Memory.ANY,
        clobbers=EVERY,
        established=False,
        evidence=(
            "rt/oscmd.asm has `cProc B$FCMD,<PUBLIC,FAR>` / `cBegin <nogen>` / `jmp B$FrameAFE`, and B$FrameAFE "
            "is nowhere in the tree. Its header claims `Modifies: NONE` and `AX = ptr to string desc for temp "
            "with command line`, which means it allocates and so can reach B$STCPCT, but a header over an "
            "unreadable body establishes nothing. Worst case, and marked as such."
        ),
    ),
)


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
_X87 = (
    Contract(
        name="B$FCMP",
        cleanup=0,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.NONE,
        reads=Memory.NONE,
        clobbers=frozenset({Reg.AX, Reg.FLAGS}),
        established=True,
        evidence=(
            "87bhelp.asm, disassembled from all three libraries and identical in each: push bp / mov bp,sp / "
            "wait / fcompp / wait / fnstsw [0] / nop / wait / mov ah,[0] / sahf / mov sp,bp / pop bp / retf. "
            "It names bx, cx, dx, si and di nowhere; ah is the only register written, and the flags ARE the "
            "return value. sahf writes SF, ZF, AF, PF and CF and cannot write OF -- it is bit 11, outside the "
            "byte -- so the comparison arrives in CF and ZF and only the unsigned branches read it, which is "
            "what wide.COMPARISONS records and what BC emits at every site. Microsoft's own runtime agrees: "
            "rt/grwindow.asm calls it and branches with JZ and JC. The fnstsw writes one word of DGROUP, the "
            "runtime's own, which is why writes is NONE the way B$DSEG's is."
        ),
    ),
    Contract(
        name="B$FILD",
        cleanup=0,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.NONE,
        reads=Memory.NONE,
        clobbers=frozenset({Reg.FLAGS}),
        established=True,
        evidence=(
            "87bhelp.asm: push bp / mov bp,sp / push bx / push dx / push ax / mov bx,sp / wait / fild dword "
            "[bx] / add sp,4 / pop bx / mov sp,bp / pop bp / retf. It takes a LONG in dx:ax and pushes it to "
            "the x87 stack through four bytes of its own frame. dx and ax are read and never written, bx is "
            "saved and restored, and nothing else is named -- so the flags `add sp,4` leaves are the only "
            "thing that changes."
        ),
    ),
    Contract(
        name="B$FIL2",
        cleanup=0,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.NONE,
        reads=Memory.NONE,
        clobbers=frozenset({Reg.DX, Reg.FLAGS}),
        established=True,
        evidence=(
            "87bhelp.asm: one instruction before B$FILD's own entry -- `cwd`, then it falls straight through. "
            "So it is the INTEGER form, taking ax alone and sign-extending it, and the cwd is exactly why dx "
            "is clobbered here and preserved there."
        ),
    ),
    Contract(
        name="B$FIST",
        cleanup=0,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.NONE,
        reads=Memory.NONE,
        clobbers=frozenset({Reg.AX, Reg.DX, Reg.FLAGS}),
        established=True,
        evidence=(
            "87bhelp.asm: push bp / mov bp,sp / sub sp,4 / wait / fistp dword [bp-4] / nop / wait / pop ax / "
            "pop dx / mov sp,bp / pop bp / retf. The LONG it produces comes back in dx:ax, which is what "
            "clobbers them; bx, cx, si and di are named nowhere."
        ),
    ),
    Contract(
        name="B$FIS2",
        cleanup=0,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.NONE,
        reads=Memory.NONE,
        clobbers=frozenset({Reg.AX, Reg.FLAGS}),
        established=True,
        evidence=(
            "87bhelp.asm: the INTEGER form of B$FIST -- sub sp,2 / fistp word [bp-2] / pop ax. One word, so "
            "ax alone comes back changed and dx does not."
        ),
    ),
    Contract(
        name="B$FUST",
        cleanup=0,
        control=Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=Memory.NONE,
        reads=Memory.NONE,
        clobbers=frozenset({Reg.AX, Reg.FLAGS}),
        established=True,
        evidence=(
            "87bhelp.asm: stores a dword, and where the high word is not zero re-loads it and stores it back "
            "as a word -- the unsigned INTEGER conversion. It ends `pop ax / add sp,2`, so only the low word "
            "is taken and dx is untouched."
        ),
    ),
)


def _contracts() -> dict[str, Contract]:
    """One entry per runtime name the corpus calls, with B$OGTA's control kind
    taken from blocks.py rather than restated here."""
    read = _HELPERS + _PRINTING + _STRINGS + _SIMPLE + _CONTROL + _FRAMES + _X87
    return {
        routine.name: (replace(routine, control=Control.INLINE_TABLE) if routine.name in INLINE_TABLE else routine)
        for routine in read
    }


CONTRACTS = _contracts()


def contract(name: str | None) -> Contract:
    """What this call can do. A name with no entry -- a user SUB or FUNCTION, a
    runtime routine nothing in the corpus has called yet, an indirect call with
    no name at all -- gets the worst case rather than an absence."""
    if name is None:
        return worst("")
    return CONTRACTS.get(name, worst(name))
