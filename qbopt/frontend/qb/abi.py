"""Late QB-family call ABI materialization.

HIR and optimizing MIR keep typed arguments on the call.  Only after those
passes have finished do stack order and cleanup become physical ARG nodes.
This is deliberately frontend-owned: no QB convention is added to MIR or the
backend.
"""

from dataclasses import replace
from dataclasses import dataclass

from iced_x86 import Register

from qbopt.model import ir
from qbopt.hir import model
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.model import memory
from qbopt.model import floating
from qbopt.backend import pointers
from qbopt.hir.lower import Lowered
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


class AbiError(ValueError):
    """A typed call lacks enough measured ABI information to emit."""


# Normal-return stack cleanup read directly from the matching entry in all
# three shipped Microsoft runtimes.  This is deliberately only a stack ABI
# table: memory, clobber, allocation, error and alias effects remain those of
# the conservative runtime contract.  BCOM45/BCL71ENR/VBDCL10E hashes are
# 5b1c7a6f..., 873fde67..., and 59ad49b0...; tools/libdump.py records the
# member, entry offset and RETF for every row.  SCAT is family-specific (the
# VBDOS far-string entry consumes an extra word), so it is not generalized.
_AUDITED_STRING_STACK: dict[str, int] = {
    "B$ASSN": 12,
    "B$FASC": 2,
    "B$FCHR": 2,
    "B$FCVD": 2,
    "B$FCVI": 2,
    "B$FCVL": 2,
    "B$FCVS": 2,
    "B$FHEX": 4,
    "B$FLEN": 2,
    "B$FMID": 6,
    "B$FMKD": 8,
    "B$FMKI": 2,
    "B$FMKL": 4,
    "B$FMKS": 4,
    "B$FMDF": 8,
    "B$FMSF": 4,
    "B$FOCT": 4,
    "B$INS2": 4,
    "B$INS3": 6,
    "B$LCAS": 2,
    "B$LDFS": 6,
    "B$LEFT": 4,
    "B$LTRM": 2,
    "B$MCVD": 2,
    "B$MCVS": 2,
    "B$RGHT": 4,
    "B$RTRM": 2,
    "B$SASS": 4,
    "B$SCMP": 4,
    "B$SPAC": 2,
    "B$STRI": 4,
    "B$STRS": 4,
    "B$UCAS": 2,
}

# Stack widths read from actual QB 4.5 /A listings for the graphics forms,
# then checked against their runtime RETF cleanup. Coordinate state is latched
# by N1/N2 before the drawing call; it is deliberately not hidden in MIR.
_AUDITED_GRAPHICS_STACK: dict[str, int] = {
    "B$PAL2": 6,
    "B$N1I2": 4,
    "B$N2I2": 4,
    "B$N1R4": 8,
    "B$N2R4": 8,
    "B$CSTT": 4,
    "B$CSTO": 4,
    "B$CASP": 4,
    # circle.asm: parmD Radius, parmW Color.
    "B$CIRC": 6,
    "B$LINE": 6,
    "B$PAIN": 4,
    "B$PSTC": 2,
    "B$PNI2": 4,
    "B$PNR4": 8,
    "B$GGET": 6,
    "B$GPUT": 8,
    "B$FTAB": 2,
}

_AUDITED_STATEMENT_STACK: dict[str, int] = {
    "B$BEEP": 0,
    "B$LNIN": 10,
    # Path descriptor, channel, record length -1 and mode; BCOM45 dkopen.asm
    # B$OPEN at 0224 returns with RETF 8 at 0252.
    "B$OPEN": 8,
    "B$SLEP": 4,
}


@dataclass(frozen=True, slots=True)
class Physicalized:
    lowered: Lowered
    calls: dict[int, str]
    contracts: dict[int, runtime.Contract]
    far_calls: frozenset[int]
    pointer_model: pointers.Model
    hints: mir.AllocationHints


def _bytes(argument: mir.Arg) -> int:
    width = argument.ref.width if isinstance(argument, mir.Cell) else getattr(argument, "width", None)
    if not isinstance(width, int) or width <= 0:
        raise AbiError(f"call argument {argument!r} has no physical width")
    return max(2, width)


def _stack_effect(reference: mir.MemRef) -> mir.MemRef:
    """Keep a call's memory identity without inventing an encoded address.

    HIR call effects retain the address values from each pointer argument so
    MIR optimization can reason about the exact source objects.  Physical ABI
    lowering has already emitted those arguments as stack writes, however: the
    call instruction itself does not read either address value in a register.
    Leaving the values on its effects makes generic instruction lowering treat
    every far selector as a hidden ES input.  B$ASSN has two far stack
    arguments, so that false requirement asked two overlapping values to
    occupy ES at the same instruction.

    This runs only after MIR optimization.  Preserve the address, provenance,
    allocation, exclusion, and source-object facts used by memory effects; only
    detach the already-materialized machine operands.
    """
    return replace(reference, base=None, segment=None)


def _stack_argument_parts(argument: mir.Arg) -> tuple[mir.Arg, ...]:
    """Spell a stored DOUBLE as two legal 386 dword pushes, high first.

    QB's Pascal stack presents the low byte of a value at ``[bp+6]``.  Since
    the stack grows downward, a multiword value is therefore pushed from its
    high end toward its low end.  HIR deliberately retains one typed 8-byte
    argument; this late source-ABI boundary is where that argument becomes
    the two physical dword pushes accepted by the real-mode backend.

    Floating source expressions are materialized to declared-width storage
    before this point, so an 8-byte argument must be an addressable cell.  A
    hidden split of an unlocated value would lose both its rounding boundary
    and its byte order, and is rejected instead.
    """
    if not isinstance(argument, mir.Cell) or argument.ref.width != 8:
        return (argument,)
    if argument.ref.addr is None:
        raise AbiError("8-byte stack argument needs addressable declared-width storage")
    return tuple(
        mir.Cell(
            replace(
                argument.ref,
                addr=argument.ref.addr.plus(offset),
                width=4,
                provenance=(argument.ref.provenance.shifted(offset) if argument.ref.provenance is not None else None),
            )
        )
        for offset in (4, 0)
    )


def _contract(name: str, cleanup: model.StackCleanup, pushed: int, family: model.RuntimeProfile) -> runtime.Contract:
    resume_label = name.startswith("$QB$RESA:")
    restore_label = name.startswith("$QB$RSTB:")
    physical_name = "B$RESA" if resume_label else "B$RSTB" if restore_label else name
    found = runtime.per_call({0: physical_name}, family.value)[0]
    if _AUDITED_STATEMENT_STACK.get(physical_name) == pushed:
        return replace(
            found,
            cleanup=pushed,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} Actual QB45 /A output supplies {pushed} stack bytes "
                f"to {name}; the shipped runtime returns past that exact fixed block. "
                "All non-stack effects remain conservative."
            ),
        )
    if _AUDITED_GRAPHICS_STACK.get(name) == pushed:
        return replace(
            found,
            cleanup=pushed,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 /A listings show {name} consuming exactly "
                f"{pushed} typed stack bytes; the shipped runtime returns past the same block. "
                "All graphics-state, memory, error, and clobber effects remain conservative."
            ),
        )
    if _AUDITED_STRING_STACK.get(name) == pushed:
        return replace(
            found,
            cleanup=pushed,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} Typed source supplies {pushed} stack bytes. "
                "BCOM45.LIB, BCL71ENR.LIB and VBDCL10E.LIB expose the same "
                f"Pascal {name} normal-return cleanup; tools/libdump.py shows "
                f"RETF {pushed} in each member. All non-stack effects remain conservative."
            ),
        )
    if resume_label and pushed == 0:
        # QB45 rt/error.asm documents B$RESA's target in AX and its exit as a
        # transfer to that address after B$RES_SETUP/B$EXSA. VBDOS LOCERR.OBJ
        # and PDS71 PDLOCAL.OBJ independently spell `mov ax,offset target`
        # immediately before a far B$RESA call. The frontend inserts that AX
        # move only after final code labels exist, so it is not a semantic
        # stack argument here.
        return replace(
            found,
            cleanup=0,
            control=runtime.Control.NEVER,
            enters_user_code=True,
            error_handling=True,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                "QB45 rt/error.asm B$RESA takes the resume offset in AX, calls "
                "B$RES_SETUP, unwinds with B$EXSA, and transfers through RESRET; "
                "measured VBDOS LOCERR.OBJ and PDS71 PDLOCAL.OBJ use the same "
                "MOV AX,offset / far-call shape."
            ),
        )
    if name == "B$RES0" and pushed == 0:
        return replace(
            found,
            cleanup=0,
            control=runtime.Control.NEVER,
            enters_user_code=True,
            error_handling=True,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                "QB45 rt/error.asm B$RES0 calls B$RES_SETUP, reloads b$erradr, "
                "unwinds with B$EXSA, and transfers through RESRET to the failing statement."
            ),
        )
    if name in {"B$RDIM", "B$DDIM"}:
        # B$ExitDim removes three header words and two words per dimension;
        # this is exactly the byte count represented by the typed operands.
        return replace(
            found,
            cleanup=pushed,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"rt/dynamic.asm {name} reaches DIM_COMMON with stack parameters; "
                "B$ExitDim removes 6 + 4*rank bytes, represented exactly by this "
                "call site's typed operands. Other effects stay conservative."
            ),
        )
    if name in {"B$COLR", "B$LOCT"} and pushed >= 2 and pushed % 2 == 0:
        return replace(
            found,
            cleanup=pushed,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"QB45 runtime/rt/gwscr.asm documents a presence word and optional value for each "
                f"positional argument, followed by their word count; this site supplies {pushed} "
                f"bytes. B$ScCleanUpParms removes that complete count-led block; later runtimes retain it."
            ),
        )
    if name == "B$HARY" and pushed >= 4 and pushed % 2 == 0:
        # The rank word plus exactly that many INTEGER subscripts are the
        # complete variable-sized stack block. The descriptor itself is a
        # fixed BX input and the result is ES:BX, established independently
        # by the PDS /Ah object probe and the shipped runtime listing.
        return replace(
            found,
            cleanup=pushed,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset({runtime.Reg.BX}),
            i386=True,
            evidence=(
                f"{found.evidence} Typed /Ah access supplies {pushed // 2 - 1} "
                f"subscripts plus rank ({pushed} stack bytes) and a separate BX descriptor; "
                "PDS71 PDHUGE.OBJ 0047..0055 shows this exact shape and consumes the "
                "returned ES:BX immediately as the element address."
            ),
        )
    if name == "B$CLOS":
        # CLOS walks a count followed by that many file words and restores SP
        # from the resulting cursor. The typed source site makes the otherwise
        # variable cleanup exact; all device/error/memory effects remain those
        # of the conservative measured contract.
        return replace(
            found,
            cleanup=pushed,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} Typed CLOSE site supplies its count and "
                f"{pushed - 2} bytes of file numbers, so normal cleanup is {pushed}. "
                "All other effects remain conservative."
            ),
        )
    if family is model.RuntimeProfile.VBDOS and name in {"B$STR4", "B$STR8"}:
        # The shared object raiser kept these conservative because it does not
        # model the mathpack's transitive stack paths. At a typed source site,
        # however, the wrapper's own RETF 4/8 and exact argument width establish
        # normal cleanup without making any stronger effect claim.
        return replace(
            found,
            cleanup=pushed,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} VBDOS stringfp.asm wrapper ends RETF {pushed}; "
                "typed source supplies that exact stored floating argument. "
                "All memory, clobber and error effects remain conservative."
            ),
        )
    if name == "B$SMID" and pushed == 12:
        return replace(
            found,
            cleanup=12,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/mid.asm declares one dword and four "
                "word parameters; cEnd returns past all 12 bytes. VBDOS listings "
                "use the same stack shape. Other effects remain conservative."
            ),
        )
    if name in {"B$GET3", "B$PUT3"} and pushed == 8:
        return replace(
            found,
            cleanup=8,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/dvstmt.asm declares Channel word, "
                "RecPtr dword, and RecLen word; B$GET3 cEnd returns past all "
                "8 bytes and B$PUT3 joins that epilogue. Other effects stay conservative."
            ),
        )
    if name in {"B$GET4", "B$PUT4"} and pushed == 12:
        return replace(
            found,
            cleanup=12,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/dvstmt.asm declares Channel word, RecNum dword, "
                "RecPtr dword, and RecLen word; both entry points return past all 12 bytes."
            ),
        )
    if name == "B$SSEK" and pushed == 6:
        return replace(
            found,
            cleanup=6,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/dkio.asm declares FileNum word and RecNum dword; "
                "B$SSEK is a far Pascal entry returning past all 6 bytes."
            ),
        )
    if family is model.RuntimeProfile.QB45 and name == "B$POKE" and pushed == 4:
        return replace(
            found,
            cleanup=4,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/peek.asm declares addr then val as two "
                "Pascal words; B$POKE ends through cEnd and normally removes all "
                "4 bytes. Memory and error effects remain conservative."
            ),
        )
    if name == "B$SERR" and pushed == 2:
        return replace(
            found,
            cleanup=2,
            control=runtime.Control.NEVER,
            enters_user_code=True,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/erproc.asm declares one word errnum, "
                "and measured VBDOS LOCERR.OBJ uses the identical one-word far "
                "call; B$SERR validates it and jumps to B$RUNERR. Its documented "
                "exit is either the installed handler or fatal termination, never "
                "the instruction after the call."
            ),
        )
    if name in {"B$STRI", "B$STRS"} and pushed == 4:
        return replace(
            found,
            cleanup=4,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/strfcn.asm declares two word parameters for {name}; "
                "the far Pascal entry returns past all 4 bytes."
            ),
        )
    if name == "B$FLEN" and pushed == 2:
        return replace(
            found,
            cleanup=2,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} Typed LEN supplies one descriptor word. "
                "BCOM45 strfcn.asm B$FLEN 000f..001c, BCL71ENR stcommon.asm "
                "0050..005d, and VBDCL10E stcore.asm 02b9..02f9 each end RETF 2. "
                "Heap, alias and error effects remain conservative."
            ),
        )
    if name == "B$SPAC" and pushed == 2:
        return replace(
            found,
            cleanup=2,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} Typed SPACE$ supplies the word count seen at "
                "[BP+6]; VBDOS D_SURF.OBJ 2d1c..2d24 shows that one pushed word, "
                "and farstr strfcn.asm's Pascal entry returns past 2 bytes. "
                "Allocation, alias and error effects remain conservative."
            ),
        )
    if name in {"B$FEVS", "B$FEVI"} and pushed == 2:
        return replace(
            found,
            cleanup=2,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/osstmt.asm declares one word descriptor or integer "
                f"parameter for {name}; the far Pascal entry returns past 2 bytes."
            ),
        )
    print_width = {
        "I2": 2,
        "I4": 4,
        "SD": 2,
        "R4": 4,
        "R8": 8,
    }
    if len(name) == 6 and name.startswith("B$P") and name[3] in "CSE":
        expected = print_width.get(name[4:])
        if expected == pushed:
            return replace(
                found,
                cleanup=expected,
                control=runtime.Control.RETURNS,
                enters_user_code=False,
                established=True,
                inputs=frozenset(),
                i386=True,
                evidence=(
                    f"{found.evidence} QB45 rt/prnval.asm and rt/prnvalfp.asm dispatch {name} "
                    f"to B$PRINT, whose typed epilogue removes the {expected}-byte value."
                ),
            )
    if (name == "B$CHOU" and pushed == 2) or (name == "B$PEOS" and pushed == 0):
        return replace(
            found,
            cleanup=pushed,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/pr0a.asm/prnval.asm establishes the typed {name} "
                f"entry with {pushed} stack bytes; output and error effects remain conservative."
            ),
        )
    if name == "B$INPP" and pushed == 6:
        return replace(
            found,
            cleanup=6,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 runtime/rt/inptty.asm declares one near prompt "
                "descriptor and one far input-table pointer; its cProc frame reads the "
                "six-byte argument block at [BP+6]..[BP+0A]. PDS and VBDOS retain the "
                "same documented entry, confirmed by the measured VBDOS INP*.OBJ sites."
            ),
        )
    if name in {"B$RDI2", "B$RDI4", "B$RDR4", "B$RDR8"} and pushed == 4:
        return replace(
            found,
            cleanup=4,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/read.asm sends {name} through CommRead with one "
                "far destination pointer; its shared cEnd removes 4 bytes."
            ),
        )
    if name in {"B$LBND", "B$UBND"} and pushed == 4:
        return replace(
            found,
            cleanup=4,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/dynamic.asm declares pAd and iDim as "
                f"two word Pascal parameters for {name}; the shared cEnd removes "
                "exactly four bytes. Bounds errors and descriptor effects remain conservative."
            ),
        )
    if name == "B$RDSD" and pushed == 6:
        return replace(
            found,
            cleanup=6,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/read.asm AssignStr consumes a far destination "
                "pointer plus a word fixed-string length; shared return removes 6 bytes."
            ),
        )
    if name == "B$FDR1" and pushed == 2:
        return replace(
            found,
            cleanup=2,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} The typed DIR$ source form selects the FDR1 "
                "find-first entry and supplies its one descriptor word; VBDCL10E "
                "rt/dkdir.asm's saved nonzero mode selects the shared RETF 2 exit. "
                "Filesystem, heap, alias and error effects remain conservative."
            ),
        )
    if name == "B$CSCN" and pushed == 6:
        return replace(
            found,
            cleanup=6,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} The audited one-mode SCREEN source form emits "
                "two count-led parameter words plus the count; SYS.OBJ 0787..0792 "
                "shows six bytes and the shared cleanup removes exactly that block. "
                "Display, alias and error effects remain conservative."
            ),
        )
    if name == "B$DSG0" and pushed == 0:
        return replace(
            found,
            cleanup=0,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45, PDS71 and VBDOS rtinit.asm B$DSG0 "
                "store DS into the runtime DEF SEG word and immediately RETF; "
                "the typed bare DEF SEG form has no stack arguments."
            ),
        )
    if name in {"B$FERR", "B$FERL"} and pushed == 0:
        return replace(
            found,
            cleanup=0,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} Typed ERR/ERL has no arguments; rt/error.asm "
                "returns directly after loading the saved value."
            ),
        )
    if name == "B$FREF" and pushed == 0:
        return replace(
            found,
            cleanup=0,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/dvstmt.asm declares `int pascal "
                "B$FREF(void)` and its cProc has no parameters; the measured "
                "PDS71 PDLOCAL.OBJ call likewise has no preceding argument push."
            ),
        )
    if name == "B$FRSD" and pushed == 2:
        return replace(
            found,
            cleanup=2,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45, PDS71 and VBDOS stfree.asm B$FRSD "
                "read the descriptor word at [BP+6] and join a RETF 2 epilogue; "
                'the FRE("") VBDOS probe pushes the canonical null descriptor. '
                "Heap and error effects remain conservative."
            ),
        )
    if name == "B$FLOF" and pushed == 2:
        return replace(
            found,
            cleanup=2,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} Typed LOF supplies the single file-number word; "
                "MAIN.OBJ 0ec4..0ecc shows that exact call shape and the runtime "
                "wrapper's normal return consumes it. Device/error effects remain conservative."
            ),
        )
    if name == "B$RNZP" and pushed == 8:
        return replace(
            found,
            cleanup=8,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} VBDCL10E.LIB random.asm B$RNZP at 0079 "
                "reads its R8 seed at [BP+0Ah] and returns with RETF 8; "
                "MAIN.OBJ 07fe..080c pushes the high and low dwords before the call."
            ),
        )
    if name == "B$RND0" and pushed == 0:
        return replace(
            found,
            cleanup=0,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 random.asm declares B$RND0 with no "
                "parameters and returns its result address in AX with a bare RETF."
            ),
        )
    if name == "B$RND1" and pushed == 4:
        return replace(
            found,
            cleanup=4,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} VBDCL10E.LIB random.asm B$RND1 reads its R4 "
                "selector at [BP+6]/[BP+8], returns its result address in AX, "
                "and exits with RETF 4; RND.OBJ shows the matching dword push."
            ),
        )
    if name == "B$RSTB" and pushed == 2:
        return replace(
            found,
            cleanup=2,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} VBDOS REST.OBJ moves the labeled DATA key "
                "to AX, pushes it, and calls B$RSTB with one word."
            ),
        )
    if name == "B$SCLS" and pushed == 2:
        return replace(
            found,
            cleanup=2,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/gwscr.asm declares B$SCLS with one "
                "ScnNum word; its Pascal far entry consumes that selector. "
                "VBDOS uses the same typed runtime entry."
            ),
        )
    if name == "B$WIDT" and pushed == 4:
        return replace(
            found,
            cleanup=4,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 runtime/rt/iotty.asm declares width and height "
                "as two Pascal words; the typed source site supplies exactly those four bytes."
            ),
        )
    if name == "B$VWPT" and pushed == 4:
        return replace(
            found,
            cleanup=4,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 runtime/rt/grview.asm declares TopLine and BotLine "
                "as two Pascal words; VBDOS VIEWP.OBJ independently confirms that call shape."
            ),
        )
    if name == "B$SPLY" and pushed == 2:
        return replace(
            found,
            cleanup=2,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 runtime/rt/gwplays.asm declares one near string "
                "descriptor; VBDOS PLAY.OBJ independently confirms that call shape."
            ),
        )
    if name == "B$INKY" and pushed == 0:
        return replace(
            found,
            cleanup=0,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 rt/stinkey.asm declares sd* pascal "
                "B$INKY(void); its far entry returns the descriptor in AX with no parameters."
            ),
        )
    if name == "B$USNG" and pushed == 2:
        return replace(
            found,
            cleanup=2,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB45 runtime/rt/prtu.asm declares one near format "
                "descriptor; VBDOS PRTUSING.OBJ independently confirms that call shape."
            ),
        )
    if found.established and found.cleanup is not None:
        return found
    if found.cleanup == pushed:
        # The object raiser may know only a conservative machine interface
        # for this runtime family even though it measured the normal RETF
        # cleanup. A typed source call establishes that all arguments are on
        # the stack, so refine only those two facts. Keep unknown control,
        # memory, callbacks, errors, and clobbers exactly as conservative as
        # the measured contract supplied them.
        return replace(
            found,
            established=True,
            inputs=frozenset(),
            i386=True,
            evidence=(
                f"{found.evidence} QB typed source call supplies {pushed} stack bytes; "
                "no register inputs are used. All other effects are unchanged."
            ),
        )
    if name.startswith("B$"):
        raise AbiError(f"runtime call {name} has no complete stack-cleanup contract")
    caller = cleanup is model.StackCleanup.CALLER
    return replace(
        runtime.worst(name),
        cleanup=0 if caller else pushed,
        caller_cleanup=pushed if caller else 0,
        control=runtime.Control.RETURNS,
        enters_user_code=False,
        established=True,
        inputs=frozenset(),
        i386=True,
        evidence=(
            "QB source ABI: stack-only far call; cleanup and argument widths "
            "come from verified HIR, while memory and clobbers remain conservative"
        ),
    )


def physicalize(program: model.Program, function: model.Function, lowered: Lowered) -> Physicalized:
    """Turn one optimized semantic body into the backend's existing call form."""
    sites = {one.instruction: one for one in function.calls}
    next_at = max((op.at for block in lowered.body.blocks for op in block.ops), default=0) + 1
    calls: dict[int, str] = {}
    contracts: dict[int, runtime.Contract] = {}
    far: set[int] = set()
    module = next(one for one in program.modules if function in one.functions)
    callable_names = {one.id: one.name for one in module.callables}
    callables = {one.id: one for one in module.callables}
    types = {one.id: one for one in module.types}
    value_types = {
        lowered.values[value.id]: types[value.type] for value in function.values if value.id in lowered.values
    }

    def call_name(operation: mir.Op) -> str:
        site = sites.get(operation.id)
        if site is None:
            raise AbiError(f"call {operation.id} has no ABI site")
        return callable_names[site.callee] if site.callee is not None else operation.name

    hary_results: dict[tuple[mir.Value, mir.Value], None] = {}
    for source_block in lowered.body.blocks:
        for source_operation in source_block.ops:
            if source_operation.kind is not mir.Kind.CALL or call_name(source_operation) != "B$HARY":
                continue
            if len(source_operation.results) != 2 or any(
                not isinstance(one, mir.Held) or one.width != 2 for one in source_operation.results
            ):
                raise AbiError("B$HARY needs explicit INTEGER offset and selector results")
            offset, selector = source_operation.results
            hary_results[(offset.value, selector.value)] = None
    hary_pointers: dict[mir.Value, tuple[mir.Value, mir.Value]] = {}
    for source_block in lowered.body.blocks:
        for source_operation in source_block.ops:
            if (
                source_operation.kind is mir.Kind.CONCAT
                and len(source_operation.args) == 2
                and len(source_operation.results) == 1
                and isinstance(source_operation.args[0], mir.Held)
                and isinstance(source_operation.args[1], mir.Held)
                and isinstance(source_operation.results[0], mir.Held)
                and (source_operation.args[1].value, source_operation.args[0].value) in hary_results
            ):
                hary_pointers[source_operation.results[0].value] = (
                    source_operation.args[1].value,
                    source_operation.args[0].value,
                )

    def hary_reference(reference: mir.MemRef) -> mir.MemRef:
        parts = hary_pointers.get(reference.base)
        if parts is None:
            return reference
        if (
            not reference.pointer
            or reference.base_width != 4
            or reference.addr is not None
            or reference.segment is not None
        ):
            raise AbiError("B$HARY result escaped as something other than one element address")
        offset, selector = parts
        return replace(
            reference,
            addr=Addr(Space.FAR, 0),
            base=offset,
            segment=selector,
            space=Space.FAR,
            base_width=2,
            pointer=False,
        )

    def hary_access(operation: mir.Op) -> mir.Op:
        replaced = {value: parts for value, parts in hary_pointers.items() if value in operation.uses}
        if not replaced:
            return operation

        def argument(one: mir.Arg) -> mir.Arg:
            return mir.Cell(hary_reference(one.ref)) if isinstance(one, mir.Cell) else one

        uses = []
        for value in operation.uses:
            uses.extend(replaced.get(value, (value,)))
        return replace(
            operation,
            uses=tuple(dict.fromkeys(uses)),
            args=tuple(map(argument, operation.args)),
            results=tuple(map(argument, operation.results)),
            loads=tuple(map(hary_reference, operation.loads)),
            stores=tuple(map(hary_reference, operation.stores)),
        )

    next_value = max((value.id for value in lowered.body.values), default=0) + 1
    origins: dict[int, int] = {}

    def fresh_value() -> mir.Value:
        nonlocal next_value
        value = mir.Value(next_value, next_at, variable=next_value, version=1)
        next_value += 1
        return value

    def long_halves(source: mir.Held) -> tuple[list[mir.Op], mir.Held, mir.Held]:
        nonlocal next_at
        halves = []
        made = []
        for offset in (0, 16):
            value = fresh_value()
            held = mir.Held(value, 2)
            halves.append(
                mir.Op(
                    next_at,
                    mir.Synth.HALF_TO_LOW,
                    "extract",
                    (value,),
                    (source.value,),
                    kind=mir.Kind.EXTRACT,
                    args=(source, mir.Const(offset, 1)),
                    results=(held,),
                    id=next_at,
                    reads_complete=True,
                    memory_complete=True,
                )
            )
            made.append(held)
            next_at += 1
        return halves, made[0], made[1]

    result_type = types[function.result_type]
    returns_legacy_long = result_type.kind is model.TypeKind.INTEGER and result_type.width == 4
    entry_loads = []
    parameter_types = [
        types[next(one for one in function.values if one.id == parameter).type] for parameter in function.parameters
    ]
    returns_legacy_float = (
        result_type.kind is model.TypeKind.FLOAT
        and function.abi is not None
        and function.abi.cleanup is model.StackCleanup.CALLEE
        and bool(function.parameters)
        and parameter_types[-1].kind is model.TypeKind.POINTER
        and parameter_types[-1].element == result_type.id
    )
    hidden_float_result = lowered.values[function.parameters[-1]] if returns_legacy_float else None
    parameter_widths = [max(2, type_.width) for type_ in parameter_types]
    # A Pascal BASIC caller evaluates and pushes left-to-right, so the first
    # source formal is furthest from the return address. CDECL pushes
    # right-to-left and therefore retains the ordinary ascending layout.
    if function.abi is not None and function.abi.cleanup is model.StackCleanup.CALLEE:
        cursor = 6 + sum(parameter_widths)
        parameter_offsets = []
        for width in parameter_widths:
            cursor -= width
            parameter_offsets.append(cursor)
    else:
        parameter_offsets = []
        cursor = 6
        for width in parameter_widths:
            parameter_offsets.append(cursor)
            cursor += width
    for number, (parameter, type_, parameter_offset) in enumerate(
        zip(function.parameters, parameter_types, parameter_offsets, strict=True)
    ):
        value = lowered.values[parameter]
        object_ = memory.Object(memory.Kind.PARAMETER, number, extent=type_.width)
        reference = mir.MemRef(
            Addr(Space.FRAME, parameter_offset),
            type_.width,
            space=Space.FRAME,
            provenance=memory.Provenance.one(object_, 0, type_.width),
        )
        result = mir.Held(value, 10 if type_.kind is model.TypeKind.FLOAT else type_.width)
        kind = mir.Kind.LOAD
        operation = ir.Operation.MOVE
        name = "mov"
        semantics = None
        if type_.kind is model.TypeKind.FLOAT:
            kind = mir.Kind.FLOAD
            operation = ir.Operation.FLOAT_LOAD
            name = "fld"
            stored = floating.Format.BINARY32 if type_.width == 4 else floating.Format.BINARY64
            semantics = floating.Semantics(
                (stored,),
                floating.Format.EXTENDED80,
                floating.Precision.EXACT,
                floating.Rounding.NONE,
            )
        entry_loads.append(
            mir.Op(
                next_at,
                operation,
                name,
                (value,),
                (),
                floating=semantics,
                loads=(reference,),
                kind=kind,
                args=(mir.Cell(reference),),
                results=(result,),
                id=next_at,
                reads_complete=True,
                memory_complete=True,
            )
        )
        next_at += 1
    blocks = []
    for block in lowered.body.blocks:
        operations = []
        for source_operation in block.ops:
            if (
                source_operation.kind is mir.Kind.CONCAT
                and len(source_operation.results) == 1
                and isinstance(source_operation.results[0], mir.Held)
                and source_operation.results[0].value in hary_pointers
            ):
                continue
            operation = hary_access(source_operation)
            if operation.kind is mir.Kind.RETURN and returns_legacy_float and len(operation.args) == 1:
                source = operation.args[0]
                if not isinstance(source, mir.Held) or source.width != 10 or hidden_float_result is None:
                    raise AbiError(f"{function.name} floating return is not one materialized value")
                pointer = mir.Held(hidden_float_result, 2)
                object_ = memory.Object(memory.Kind.PARAMETER, (function.id, "float-result"))
                reference = mir.MemRef(
                    Addr(Space.LITERAL, 0),
                    result_type.width,
                    base=hidden_float_result,
                    space=Space.LITERAL,
                    base_width=2,
                    provenance=memory.Provenance.one(object_),
                )
                stored = floating.Semantics(
                    (floating.Format.EXTENDED80,),
                    floating.Format.BINARY32 if result_type.width == 4 else floating.Format.BINARY64,
                    floating.Precision.DESTINATION,
                    floating.Rounding.DYNAMIC,
                )
                operations.append(
                    mir.Op(
                        next_at,
                        ir.Operation.FLOAT_STORE,
                        "fstp",
                        (),
                        (source.value, hidden_float_result),
                        floating=stored,
                        stores=(reference,),
                        kind=mir.Kind.FSTORE,
                        args=(source,),
                        results=(mir.Cell(reference),),
                        id=next_at,
                        reads_complete=True,
                        memory_complete=True,
                    )
                )
                next_at += 1
                operations.append(replace(operation, args=(pointer,), uses=(hidden_float_result,)))
                continue
            if operation.kind is mir.Kind.RETURN and returns_legacy_long and len(operation.args) == 1:
                source = operation.args[0]
                if not isinstance(source, mir.Held) or source.width != 4:
                    raise AbiError(f"{function.name} LONG return is not one materialized dword")
                halves, low, high = long_halves(source)
                operations.extend(halves)
                operations.append(replace(operation, args=(low, high), uses=(low.value, high.value)))
                continue
            if operation.kind is not mir.Kind.CALL:
                operations.append(operation)
                continue
            site = sites.get(operation.id)
            if site is None:
                raise AbiError(f"call {operation.id} has no ABI site")
            name = call_name(operation)
            ordered = tuple(operation.args[index] for index in site.order)
            fixed_arguments: tuple[mir.Arg, ...] = ()
            stack_arguments = ordered
            if name == "B$HARY":
                if len(ordered) < 3 or _bytes(ordered[-1]) != 2:
                    raise AbiError("B$HARY needs subscripts, rank, and one near descriptor")
                stack_arguments = ordered[:-1]
                fixed_arguments = ordered[-1:]
            pushed = sum(_bytes(one) for one in stack_arguments)
            for argument in stack_arguments:
                for part in _stack_argument_parts(argument):
                    uses = (part.value,) if isinstance(part, mir.Held) else ()
                    if isinstance(part, mir.Cell) and part.ref.base is not None:
                        uses = (part.ref.base,)
                    operations.append(
                        mir.Op(
                            next_at,
                            ir.Operation.PUSH,
                            "push",
                            (),
                            uses,
                            kind=mir.Kind.ARG,
                            args=(part,),
                            id=next_at,
                            reads_complete=True,
                        )
                    )
                    next_at += 1
            calls[operation.at] = name
            contracts[operation.at] = _contract(name, site.cleanup, pushed, program.runtime)
            if site.distance is model.CallDistance.FAR:
                far.add(operation.at)
            operation = replace(
                operation,
                loads=tuple(_stack_effect(one) for one in operation.loads),
                stores=tuple(_stack_effect(one) for one in operation.stores),
            )
            result = operation.results[0] if len(operation.results) == 1 else None
            callable_ = callables.get(site.callee) if site.callee is not None else None
            semantic_type = (
                types[callable_.result_type]
                if callable_ is not None and callable_.result_type is not None
                else value_types.get(result.value)
                if isinstance(result, mir.Held)
                else None
            )
            legacy_long = (
                isinstance(result, mir.Held)
                and result.width == 4
                and semantic_type is not None
                and semantic_type.kind is model.TypeKind.INTEGER
                and semantic_type.width == 4
            )
            legacy_float = (
                isinstance(result, mir.Held)
                and result.width == 10
                and callable_ is not None
                and semantic_type is not None
                and semantic_type.kind is model.TypeKind.FLOAT
                and site.cleanup is model.StackCleanup.CALLEE
            )
            if name == "B$HARY":
                if len(operation.results) != 2 or any(
                    not isinstance(one, mir.Held) or one.width != 2 for one in operation.results
                ):
                    raise AbiError("B$HARY needs explicit INTEGER offset and selector results")
                offset, selector = operation.results
                origins[offset.value.variable] = Register.BX
                origins[selector.value.variable] = Register.ES
                operations.append(
                    replace(
                        operation,
                        args=fixed_arguments,
                        uses=tuple(one.value for one in fixed_arguments if isinstance(one, mir.Held)),
                    )
                )
                continue
            if legacy_float:
                pointer_value = fresh_value()
                pointer = mir.Held(pointer_value, 2)
                operations.append(
                    replace(
                        operation,
                        defines=(pointer_value,),
                        results=(pointer,),
                        args=(),
                        uses=(),
                    )
                )
                object_ = memory.Object(memory.Kind.PARAMETER, (operation.id, "float-result"))
                reference = mir.MemRef(
                    Addr(Space.LITERAL, 0),
                    semantic_type.width,
                    base=pointer_value,
                    space=Space.LITERAL,
                    base_width=2,
                    provenance=memory.Provenance.one(object_),
                )
                loaded = floating.Semantics(
                    (floating.Format.BINARY32 if semantic_type.width == 4 else floating.Format.BINARY64,),
                    floating.Format.EXTENDED80,
                    floating.Precision.EXACT,
                    floating.Rounding.NONE,
                )
                operations.append(
                    mir.Op(
                        next_at,
                        ir.Operation.FLOAT_LOAD,
                        "fld",
                        (result.value,),
                        (pointer_value,),
                        floating=loaded,
                        loads=(reference,),
                        kind=mir.Kind.FLOAD,
                        args=(mir.Cell(reference),),
                        results=(result,),
                        id=next_at,
                        reads_complete=True,
                        memory_complete=True,
                    )
                )
                next_at += 1
                continue
            if not legacy_long:
                operations.append(replace(operation, args=(), uses=()))
                continue
            low, high = fresh_value(), fresh_value()
            delivered = replace(
                operation,
                defines=(low, high),
                results=(mir.Held(low, 2), mir.Held(high, 2)),
                args=(),
                uses=(),
            )
            operations.append(delivered)
            operations.append(
                mir.Op(
                    next_at,
                    ir.Operation.MOVE,
                    "",
                    (result.value,),
                    (high, low),
                    kind=mir.Kind.CONCAT,
                    args=(mir.Held(high, 2), mir.Held(low, 2)),
                    results=(result,),
                    id=next_at,
                    reads_complete=True,
                    memory_complete=True,
                )
            )
            next_at += 1
        if block.at == lowered.body.entry:
            operations = [*entry_loads, *operations]
        blocks.append(replace(block, ops=tuple(operations)))
    # HIR represents a source-level terminal statement with an explicit
    # UNREACHABLE terminator. Once this frontend's audited call contract says
    # the preceding runtime call never returns, that marker has no machine
    # operation and no remaining control-flow meaning. Keep it out of LIR;
    # emitting a placeholder after B$CEND/B$RESA also makes later block
    # placement mistake data-less syntax for a real instruction.
    blocks = [
        replace(block, ops=block.ops[:-1])
        if len(block.ops) >= 2
        and block.ops[-1].kind is mir.Kind.ESCAPE
        and block.ops[-2].kind is mir.Kind.CALL
        and contracts.get(block.ops[-2].at) is not None
        and contracts[block.ops[-2].at].control is runtime.Control.NEVER
        else block
        for block in blocks
    ]
    body = replace(lowered.body, blocks=tuple(blocks))
    checked = body
    external = tuple(
        dict.fromkeys(
            (
                body.entry,
                *function.external_entries,
                *(() if function.error_handler is None else (function.error_handler,)),
            )
        )
    )
    if len(external) > 1:
        root = max(block.at for block in body.blocks) + 1
        checked = replace(
            body,
            entry=root,
            blocks=(mir.MirBlock(root, (), (), external), *body.blocks),
        )
    problems = mir.verify(checked)
    if problems:
        raise AbiError(f"physicalized {lowered.name} is invalid: {problems[:3]}")
    # DOS huge pointers advance their selector by 1 << 12 per wrapped 64 KiB
    # page (runtime nhinit.asm). Far accesses use the same established model
    # even when no arithmetic crosses a page.
    return Physicalized(
        replace(lowered, body=body),
        calls,
        contracts,
        frozenset(far),
        pointers.Model(12),
        mir.AllocationHints(origins=origins),
    )
