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
import tomllib
from pathlib import Path
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
