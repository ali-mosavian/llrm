//! The QB-family runtime's call ABI, as selection asks it: each callee's
//! contract (stack cleanup, clobbers, register arguments), its linked name,
//! and the CPU target the backend lowers to. MIR calls carry typed
//! arguments only; no QB convention reaches MIR.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::LazyLock;

use iced_x86::Register;

use crate::abi::runtime::{self, Contract, Control};
use crate::backend::assemble::Registers;
use crate::hir::model;
use crate::support::hash::IndexMap;
use crate::support::pyrepr::{self};

/// A typed call lacks enough measured ABI information to emit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbiError(pub String);

impl fmt::Display for AbiError {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AbiError {}

// Normal-return stack cleanup read directly from the matching entry in all
// three shipped Microsoft runtimes.  This is deliberately only a stack ABI
// table: memory, clobber, allocation, error and alias effects remain those of
// the conservative runtime contract.  BCOM45/BCL71ENR/VBDCL10E hashes are
// 5b1c7a6f..., 873fde67..., and 59ad49b0...; tools/libdump.py records the
// member, entry offset and RETF for every row.  SCAT is family-specific (the
// VBDOS far-string entry consumes an extra word), so it is not generalized.
static _AUDITED_STRING_STACK: LazyLock<IndexMap<&str, i64>> = LazyLock::new(|| {
    IndexMap::from_iter([
        ("B$ASSN", 12),
        ("B$FASC", 2),
        ("B$FCHR", 2),
        ("B$FCVD", 2),
        ("B$FCVI", 2),
        ("B$FCVL", 2),
        ("B$FCVS", 2),
        ("B$FHEX", 4),
        ("B$FLEN", 2),
        ("B$FMID", 6),
        ("B$FMKD", 8),
        ("B$FMKI", 2),
        ("B$FMKL", 4),
        ("B$FMKS", 4),
        ("B$FMDF", 8),
        ("B$FMSF", 4),
        ("B$FOCT", 4),
        ("B$INS2", 4),
        ("B$INS3", 6),
        ("B$LCAS", 2),
        ("B$LDFS", 6),
        ("B$LEFT", 4),
        ("B$LTRM", 2),
        ("B$MCVD", 2),
        ("B$MCVS", 2),
        ("B$RGHT", 4),
        ("B$RTRM", 2),
        ("B$SASS", 4),
        ("B$SCMP", 4),
        ("B$SCPY", 2),
        ("B$SPAC", 2),
        ("B$STRI", 4),
        ("B$STRS", 4),
        ("B$UCAS", 2),
    ])
});

// Stack widths read from actual QB 4.5 /A listings for the graphics forms,
// then checked against their runtime RETF cleanup. Coordinate state is latched
// by N1/N2 before the drawing call; it is deliberately not hidden in MIR.
static _AUDITED_GRAPHICS_STACK: LazyLock<IndexMap<&str, i64>> = LazyLock::new(|| {
    IndexMap::from_iter([
        ("B$PAL2", 6),
        ("B$N1I2", 4),
        ("B$N2I2", 4),
        ("B$N1R4", 8),
        ("B$N2R4", 8),
        ("B$CSTT", 4),
        ("B$CSTO", 4),
        ("B$CASP", 4),
        // circle.asm: parmD Radius, parmW Color.
        ("B$CIRC", 6),
        ("B$LINE", 6),
        ("B$PAIN", 4),
        ("B$PSTC", 2),
        ("B$PNI2", 4),
        ("B$PNR4", 8),
        ("B$GGET", 6),
        ("B$GPUT", 8),
        ("B$FTAB", 2),
    ])
});

static _AUDITED_STATEMENT_STACK: LazyLock<IndexMap<&str, i64>> = LazyLock::new(|| {
    IndexMap::from_iter([
        ("B$BEEP", 0),
        ("B$LNIN", 10),
        // `void B$LPRT(void)` and `void B$WRIT(void)`: QB45 rt/iolpt.asm and
        // rt/prnval.asm, FAR, no arguments.
        ("B$LPRT", 0),
        ("B$WRIT", 0),
        // Path descriptor, channel, record length -1 and mode; BCOM45
        // dkopen.asm B$OPEN at 0224 returns with RETF 8 at 0252.
        ("B$OPEN", 8),
        ("B$SLEP", 4),
    ])
});

/// An inline block as a call: its declared registers are all it reads and
/// changes.
fn _inline_contract(
    asm: &model::Asm
) -> Result<(Contract, Vec<(runtime::Reg, u32)>, Vec<(runtime::Reg, u32)>), AbiError> {
    let registers = |names: &[String]| {
        names
            .iter()
            .map(|name| {
                crate::backend::inline_asm::view(name)
                    .ok_or_else(|| AbiError(format!("inline assembly names no register {}", pyrepr::string(name))))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let (inputs, outputs, clobbers) = (registers(&asm.inputs)?, registers(&asm.outputs)?, registers(&asm.clobbers)?);
    // A register is its root whatever the view: a 32-bit one clobbers the
    // whole.
    let roots = |named: &[(runtime::Reg, u32)]| named.iter().map(|one| one.0).collect::<Vec<_>>();
    let (input_roots, output_roots, clobber_roots) = (roots(&inputs), roots(&outputs), roots(&clobbers));
    let memory = if asm.memory { runtime::Memory::Any } else { runtime::Memory::None };
    let contract = Contract {
        name: crate::hir::symbols::ASM.to_owned(),
        cleanup: Some(0),
        control: Control::Returns,
        enters_user_code: false,
        raises_error: false,
        error_handling: false,
        writes: memory,
        reads: memory,
        clobbers: clobber_roots.iter().chain(&output_roots).copied().collect(),
        established: true,
        evidence: "inline assembly: the inputs, outputs and clobbers it declares".to_owned(),
        documented: None,
        inputs: Some(input_roots.iter().copied().collect()),
        direct_inputs: None,
        clobbers_reached: true,
        caller_cleanup: 0,
        // It names no 32-bit register, so it keeps every high half.
        i386: false,
        direct_writes: None,
        flags_result: false,
        direct_reads: None,
    };
    Ok((contract, inputs, outputs))
}

/// The contract and registers of the inline block an intrinsic name spells,
/// as `llrm_mir::intrinsics::asm` reads it; none for another name.
pub fn asm_call(name: &str) -> Option<Result<(Contract, Registers), AbiError>> {
    let block = llrm_mir::intrinsics::asm(name)?;
    let asm = model::Asm {
        code: block.code.iter().map(|&byte| i64::from(byte)).collect(),
        inputs: block.inputs,
        outputs: block.outputs,
        clobbers: block.clobbers,
        memory: block.memory,
    };
    Some(_inline_contract(&asm).and_then(|(contract, inputs, outputs)| {
        let machine = |names: Vec<(runtime::Reg, u32)>| {
            names
                .into_iter()
                .map(|(one, bits)| match one {
                    runtime::Reg::Ax
                    | runtime::Reg::Bx
                    | runtime::Reg::Cx
                    | runtime::Reg::Dx
                    | runtime::Reg::Si
                    | runtime::Reg::Di => crate::backend::inline_asm::machine(one, bits)
                        .ok_or_else(|| AbiError(format!("inline assembly passes a value in {}", one.name()))),
                    other => Err(AbiError(format!("inline assembly passes a value in {}", other.name()))),
                })
                .collect::<Result<Vec<_>, _>>()
        };
        Ok((contract, Registers { arguments: machine(inputs)?, results: machine(outputs)? }))
    }))
}

pub fn _contract(
    name: &str,
    cleanup: model::StackCleanup,
    pushed: i64,
    family: model::RuntimeProfile,
) -> Result<Contract, AbiError> {
    _contract_keeping(name, cleanup, pushed, family, &BTreeSet::new())
}

/// `_contract`, a call no runtime contract describes keeping `preserved`.
fn _contract_keeping(
    name: &str,
    cleanup: model::StackCleanup,
    pushed: i64,
    family: model::RuntimeProfile,
    preserved: &BTreeSet<runtime::Reg>,
) -> Result<Contract, AbiError> {
    let resume_label = name.starts_with("$QB$RESA:");
    let restore_label = name.starts_with("$QB$RSTB:");
    let physical_name = if resume_label {
        "B$RESA"
    } else if restore_label {
        "B$RSTB"
    } else {
        name
    };
    let found =
        runtime::per_call(&IndexMap::from_iter([(0, physical_name.to_owned())]), family.value(), &BTreeSet::new())
            .swap_remove(&0)
            .expect("one call in, one contract out");
    let evidence = &found.evidence;
    // `replace(found, cleanup=..., control=..., enters_user_code=...,
    // established=True, inputs=frozenset(), i386=True, evidence=...)`.
    let refined =
        |found: &Contract, cleanup: i64, control: Control, enters_user_code: bool, evidence: String| Contract {
            cleanup: Some(cleanup),
            control,
            enters_user_code,
            established: true,
            inputs: Some(BTreeSet::new()),
            i386: true,
            evidence,
            ..found.clone()
        };
    let returns = |cleanup: i64, text: String| refined(&found, cleanup, Control::Returns, false, text);
    if _AUDITED_STATEMENT_STACK.get(physical_name) == Some(&pushed) {
        return Ok(returns(
            pushed,
            format!(
                "{evidence} Actual QB45 /A output supplies {pushed} stack bytes \
                 to {name}; the shipped runtime returns past that exact fixed block. \
                 All non-stack effects remain conservative."
            ),
        ));
    }
    if _AUDITED_GRAPHICS_STACK.get(name) == Some(&pushed) {
        return Ok(returns(
            pushed,
            format!(
                "{evidence} QB45 /A listings show {name} consuming exactly \
                 {pushed} typed stack bytes; the shipped runtime returns past the same block. \
                 All graphics-state, memory, error, and clobber effects remain conservative."
            ),
        ));
    }
    if _AUDITED_STRING_STACK.get(name) == Some(&pushed) {
        return Ok(returns(
            pushed,
            format!(
                "{evidence} Typed source supplies {pushed} stack bytes. \
                 BCOM45.LIB, BCL71ENR.LIB and VBDCL10E.LIB expose the same \
                 Pascal {name} normal-return cleanup; tools/libdump.py shows \
                 RETF {pushed} in each member. All non-stack effects remain conservative."
            ),
        ));
    }
    if resume_label && pushed == 0 {
        // QB45 rt/error.asm documents B$RESA's target in AX and its exit as a
        // transfer to that address after B$RES_SETUP/B$EXSA. VBDOS LOCERR.OBJ
        // and PDS71 PDLOCAL.OBJ independently spell `mov ax,offset target`
        // immediately before a far B$RESA call. The frontend inserts that AX
        // move only after final code labels exist, so it is not a semantic
        // stack argument here.
        return Ok(Contract {
            error_handling: true,
            ..refined(
                &found,
                0,
                Control::Never,
                true,
                "QB45 rt/error.asm B$RESA takes the resume offset in AX, calls \
                 B$RES_SETUP, unwinds with B$EXSA, and transfers through RESRET; \
                 measured VBDOS LOCERR.OBJ and PDS71 PDLOCAL.OBJ use the same \
                 MOV AX,offset / far-call shape."
                    .to_owned(),
            )
        });
    }
    if name == "B$RES0" && pushed == 0 {
        return Ok(Contract {
            error_handling: true,
            ..refined(
                &found,
                0,
                Control::Never,
                true,
                "QB45 rt/error.asm B$RES0 calls B$RES_SETUP, reloads b$erradr, \
                 unwinds with B$EXSA, and transfers through RESRET to the failing statement."
                    .to_owned(),
            )
        });
    }
    if name == "B$RDIM" || name == "B$DDIM" {
        // B$ExitDim removes three header words and two words per dimension;
        // this is exactly the byte count represented by the typed operands.
        return Ok(returns(
            pushed,
            format!(
                "rt/dynamic.asm {name} reaches DIM_COMMON with stack parameters; \
                 B$ExitDim removes 6 + 4*rank bytes, represented exactly by this \
                 call site's typed operands. Other effects stay conservative."
            ),
        ));
    }
    if (name == "B$COLR" || name == "B$LOCT") && pushed >= 2 && pushed % 2 == 0 {
        return Ok(returns(
            pushed,
            format!(
                "QB45 runtime/rt/gwscr.asm documents a presence word and optional value for each \
                 positional argument, followed by their word count; this site supplies {pushed} \
                 bytes. B$ScCleanUpParms removes that complete count-led block; later runtimes retain it."
            ),
        ));
    }
    if name == "B$HARY" && pushed >= 4 && pushed % 2 == 0 {
        // The rank word plus exactly that many INTEGER subscripts are the
        // complete variable-sized stack block. The descriptor itself is a
        // fixed BX input and the result is ES:BX, established independently
        // by the PDS /Ah object probe and the shipped runtime listing.
        return Ok(Contract {
            inputs: Some(BTreeSet::from([runtime::Reg::Bx])),
            ..returns(
                pushed,
                format!(
                    "{evidence} Typed /Ah access supplies {} \
                     subscripts plus rank ({pushed} stack bytes) and a separate BX descriptor; \
                     PDS71 PDHUGE.OBJ 0047..0055 shows this exact shape and consumes the \
                     returned ES:BX immediately as the element address.",
                    pushed / 2 - 1
                ),
            )
        });
    }
    if name == "B$CLOS" {
        // CLOS walks a count followed by that many file words and restores SP
        // from the resulting cursor. The typed source site makes the otherwise
        // variable cleanup exact; all device/error/memory effects remain those
        // of the conservative measured contract.
        return Ok(returns(
            pushed,
            format!(
                "{evidence} Typed CLOSE site supplies its count and \
                 {} bytes of file numbers, so normal cleanup is {pushed}. \
                 All other effects remain conservative.",
                pushed - 2
            ),
        ));
    }
    if family == model::RuntimeProfile::Vbdos && (name == "B$STR4" || name == "B$STR8") {
        // The shared object raiser kept these conservative because it does not
        // model the mathpack's transitive stack paths. At a typed source site,
        // however, the wrapper's own RETF 4/8 and exact argument width
        // establish normal cleanup without making any stronger effect
        // claim.
        return Ok(returns(
            pushed,
            format!(
                "{evidence} VBDOS stringfp.asm wrapper ends RETF {pushed}; \
                 typed source supplies that exact stored floating argument. \
                 All memory, clobber and error effects remain conservative."
            ),
        ));
    }
    if name == "B$SMID" && pushed == 12 {
        return Ok(returns(
            12,
            format!(
                "{evidence} QB45 rt/mid.asm declares one dword and four \
                 word parameters; cEnd returns past all 12 bytes. VBDOS listings \
                 use the same stack shape. Other effects remain conservative."
            ),
        ));
    }
    if (name == "B$GET3" || name == "B$PUT3") && pushed == 8 {
        return Ok(returns(
            8,
            format!(
                "{evidence} QB45 rt/dvstmt.asm declares Channel word, \
                 RecPtr dword, and RecLen word; B$GET3 cEnd returns past all \
                 8 bytes and B$PUT3 joins that epilogue. Other effects stay conservative."
            ),
        ));
    }
    if (name == "B$GET4" || name == "B$PUT4") && pushed == 12 {
        return Ok(returns(
            12,
            format!(
                "{evidence} QB45 rt/dvstmt.asm declares Channel word, RecNum dword, \
                 RecPtr dword, and RecLen word; both entry points return past all 12 bytes."
            ),
        ));
    }
    if name == "B$SSEK" && pushed == 6 {
        return Ok(returns(
            6,
            format!(
                "{evidence} QB45 rt/dkio.asm declares FileNum word and RecNum dword; \
                 B$SSEK is a far Pascal entry returning past all 6 bytes."
            ),
        ));
    }
    if family == model::RuntimeProfile::Qb45 && name == "B$POKE" && pushed == 4 {
        return Ok(returns(
            4,
            format!(
                "{evidence} QB45 rt/peek.asm declares addr then val as two \
                 Pascal words; B$POKE ends through cEnd and normally removes all \
                 4 bytes. Memory and error effects remain conservative."
            ),
        ));
    }
    if name == "B$SERR" && pushed == 2 {
        return Ok(refined(
            &found,
            2,
            Control::Never,
            true,
            format!(
                "{evidence} QB45 rt/erproc.asm declares one word errnum, \
                 and measured VBDOS LOCERR.OBJ uses the identical one-word far \
                 call; B$SERR validates it and jumps to B$RUNERR. Its documented \
                 exit is either the installed handler or fatal termination, never \
                 the instruction after the call."
            ),
        ));
    }
    if (name == "B$STRI" || name == "B$STRS") && pushed == 4 {
        return Ok(returns(
            4,
            format!(
                "{evidence} QB45 rt/strfcn.asm declares two word parameters for {name}; \
                 the far Pascal entry returns past all 4 bytes."
            ),
        ));
    }
    if name == "B$FLEN" && pushed == 2 {
        return Ok(returns(
            2,
            format!(
                "{evidence} Typed LEN supplies one descriptor word. \
                 BCOM45 strfcn.asm B$FLEN 000f..001c, BCL71ENR stcommon.asm \
                 0050..005d, and VBDCL10E stcore.asm 02b9..02f9 each end RETF 2. \
                 Heap, alias and error effects remain conservative."
            ),
        ));
    }
    if name == "B$SPAC" && pushed == 2 {
        return Ok(returns(
            2,
            format!(
                "{evidence} Typed SPACE$ supplies the word count seen at \
                 [BP+6]; VBDOS D_SURF.OBJ 2d1c..2d24 shows that one pushed word, \
                 and farstr strfcn.asm's Pascal entry returns past 2 bytes. \
                 Allocation, alias and error effects remain conservative."
            ),
        ));
    }
    if (name == "B$FEVS" || name == "B$FEVI") && pushed == 2 {
        return Ok(returns(
            2,
            format!(
                "{evidence} QB45 rt/osstmt.asm declares one word descriptor or integer \
                 parameter for {name}; the far Pascal entry returns past 2 bytes."
            ),
        ));
    }
    let print_width = |suffix: &str| match suffix {
        "I2" => Some(2),
        "I4" => Some(4),
        "SD" => Some(2),
        "R4" => Some(4),
        "R8" => Some(8),
        _ => None,
    };
    if name.len() == 6 && name.starts_with("B$P") && "CSE".contains(&name[3..4]) {
        let expected = print_width(&name[4..]);
        if expected == Some(pushed) {
            return Ok(returns(
                pushed,
                format!(
                    "{evidence} QB45 rt/prnval.asm and rt/prnvalfp.asm dispatch {name} \
                     to B$PRINT, whose typed epilogue removes the {pushed}-byte value."
                ),
            ));
        }
    }
    if (name == "B$CHOU" && pushed == 2) || (name == "B$PEOS" && pushed == 0) {
        return Ok(returns(
            pushed,
            format!(
                "{evidence} QB45 rt/pr0a.asm/prnval.asm establishes the typed {name} \
                 entry with {pushed} stack bytes; output and error effects remain conservative."
            ),
        ));
    }
    if name == "B$INPP" && pushed == 6 {
        return Ok(returns(
            6,
            format!(
                "{evidence} QB45 runtime/rt/inptty.asm declares one near prompt \
                 descriptor and one far input-table pointer; its cProc frame reads the \
                 six-byte argument block at [BP+6]..[BP+0A]. PDS and VBDOS retain the \
                 same documented entry, confirmed by the measured VBDOS INP*.OBJ sites."
            ),
        ));
    }
    if ["B$RDI2", "B$RDI4", "B$RDR4", "B$RDR8"].contains(&name) && pushed == 4 {
        return Ok(returns(
            4,
            format!(
                "{evidence} QB45 rt/read.asm sends {name} through CommRead with one \
                 far destination pointer; its shared cEnd removes 4 bytes."
            ),
        ));
    }
    if (name == "B$LBND" || name == "B$UBND") && pushed == 4 {
        return Ok(returns(
            4,
            format!(
                "{evidence} QB45 rt/dynamic.asm declares pAd and iDim as \
                 two word Pascal parameters for {name}; the shared cEnd removes \
                 exactly four bytes. Bounds errors and descriptor effects remain conservative."
            ),
        ));
    }
    if name == "B$RDSD" && pushed == 6 {
        return Ok(returns(
            6,
            format!(
                "{evidence} QB45 rt/read.asm AssignStr consumes a far destination \
                 pointer plus a word fixed-string length; shared return removes 6 bytes."
            ),
        ));
    }
    if name == "B$FDR1" && pushed == 2 {
        return Ok(returns(
            2,
            format!(
                "{evidence} The typed DIR$ source form selects the FDR1 \
                 find-first entry and supplies its one descriptor word; VBDCL10E \
                 rt/dkdir.asm's saved nonzero mode selects the shared RETF 2 exit. \
                 Filesystem, heap, alias and error effects remain conservative."
            ),
        ));
    }
    if name == "B$CSCN" && pushed == 6 {
        return Ok(returns(
            6,
            format!(
                "{evidence} The audited one-mode SCREEN source form emits \
                 two count-led parameter words plus the count; SYS.OBJ 0787..0792 \
                 shows six bytes and the shared cleanup removes exactly that block. \
                 Display, alias and error effects remain conservative."
            ),
        ));
    }
    if name == "B$DSG0" && pushed == 0 {
        return Ok(returns(
            0,
            format!(
                "{evidence} QB45, PDS71 and VBDOS rtinit.asm B$DSG0 \
                 store DS into the runtime DEF SEG word and immediately RETF; \
                 the typed bare DEF SEG form has no stack arguments."
            ),
        ));
    }
    if (name == "B$FERR" || name == "B$FERL") && pushed == 0 {
        return Ok(returns(
            0,
            format!(
                "{evidence} Typed ERR/ERL has no arguments; rt/error.asm \
                 returns directly after loading the saved value."
            ),
        ));
    }
    if name == "B$FREF" && pushed == 0 {
        return Ok(returns(
            0,
            format!(
                "{evidence} QB45 rt/dvstmt.asm declares `int pascal \
                 B$FREF(void)` and its cProc has no parameters; the measured \
                 PDS71 PDLOCAL.OBJ call likewise has no preceding argument push."
            ),
        ));
    }
    if name == "B$FRSD" && pushed == 2 {
        return Ok(returns(
            2,
            format!(
                "{evidence} QB45, PDS71 and VBDOS stfree.asm B$FRSD \
                 read the descriptor word at [BP+6] and join a RETF 2 epilogue; \
                 the FRE(\"\") VBDOS probe pushes the canonical null descriptor. \
                 Heap and error effects remain conservative."
            ),
        ));
    }
    if name == "B$FLOF" && pushed == 2 {
        return Ok(returns(
            2,
            format!(
                "{evidence} Typed LOF supplies the single file-number word; \
                 MAIN.OBJ 0ec4..0ecc shows that exact call shape and the runtime \
                 wrapper's normal return consumes it. Device/error effects remain conservative."
            ),
        ));
    }
    if name == "B$RNZP" && pushed == 8 {
        return Ok(returns(
            8,
            format!(
                "{evidence} VBDCL10E.LIB random.asm B$RNZP at 0079 \
                 reads its R8 seed at [BP+0Ah] and returns with RETF 8; \
                 MAIN.OBJ 07fe..080c pushes the high and low dwords before the call."
            ),
        ));
    }
    if name == "B$RND0" && pushed == 0 {
        return Ok(returns(
            0,
            format!(
                "{evidence} QB45 random.asm declares B$RND0 with no \
                 parameters and returns its result address in AX with a bare RETF."
            ),
        ));
    }
    if name == "B$RND1" && pushed == 4 {
        return Ok(returns(
            4,
            format!(
                "{evidence} VBDCL10E.LIB random.asm B$RND1 reads its R4 \
                 selector at [BP+6]/[BP+8], returns its result address in AX, \
                 and exits with RETF 4; RND.OBJ shows the matching dword push."
            ),
        ));
    }
    if name == "B$RSTB" && pushed == 2 {
        return Ok(returns(
            2,
            format!(
                "{evidence} VBDOS REST.OBJ moves the labeled DATA key \
                 to AX, pushes it, and calls B$RSTB with one word."
            ),
        ));
    }
    if name == "B$SCLS" && pushed == 2 {
        return Ok(returns(
            2,
            format!(
                "{evidence} QB45 rt/gwscr.asm declares B$SCLS with one \
                 ScnNum word; its Pascal far entry consumes that selector. \
                 VBDOS uses the same typed runtime entry."
            ),
        ));
    }
    if name == "B$WIDT" && pushed == 4 {
        return Ok(returns(
            4,
            format!(
                "{evidence} QB45 runtime/rt/iotty.asm declares width and height \
                 as two Pascal words; the typed source site supplies exactly those four bytes."
            ),
        ));
    }
    if name == "B$VWPT" && pushed == 4 {
        return Ok(returns(
            4,
            format!(
                "{evidence} QB45 runtime/rt/grview.asm declares TopLine and BotLine \
                 as two Pascal words; VBDOS VIEWP.OBJ independently confirms that call shape."
            ),
        ));
    }
    if name == "B$SPLY" && pushed == 2 {
        return Ok(returns(
            2,
            format!(
                "{evidence} QB45 runtime/rt/gwplays.asm declares one near string \
                 descriptor; VBDOS PLAY.OBJ independently confirms that call shape."
            ),
        ));
    }
    if name == "B$INKY" && pushed == 0 {
        return Ok(returns(
            0,
            format!(
                "{evidence} QB45 rt/stinkey.asm declares sd* pascal \
                 B$INKY(void); its far entry returns the descriptor in AX with no parameters."
            ),
        ));
    }
    if name == "B$USNG" && pushed == 2 {
        return Ok(returns(
            2,
            format!(
                "{evidence} QB45 runtime/rt/prtu.asm declares one near format \
                 descriptor; VBDOS PRTUSING.OBJ independently confirms that call shape."
            ),
        ));
    }
    if found.established && found.cleanup.is_some() {
        return Ok(found);
    }
    if found.cleanup == Some(pushed) {
        // The object raiser may know only a conservative machine interface
        // for this runtime family even though it measured the normal RETF
        // cleanup. A typed source call establishes that all arguments are on
        // the stack, so refine only those two facts. Keep unknown control,
        // memory, callbacks, errors, and clobbers exactly as conservative as
        // the measured contract supplied them.
        return Ok(Contract {
            established: true,
            inputs: Some(BTreeSet::new()),
            i386: true,
            evidence: format!(
                "{evidence} QB typed source call supplies {pushed} stack bytes; \
                 no register inputs are used. All other effects are unchanged."
            ),
            ..found.clone()
        });
    }
    if name.starts_with("B$") {
        return Err(AbiError(format!("runtime call {name} has no complete stack-cleanup contract")));
    }
    let caller = cleanup == model::StackCleanup::Caller;
    Ok(Contract {
        cleanup: Some(if caller { 0 } else { pushed }),
        caller_cleanup: if caller { pushed } else { 0 },
        control: Control::Returns,
        enters_user_code: false,
        established: true,
        inputs: Some(BTreeSet::new()),
        i386: true,
        evidence: "QB source ABI: stack-only far call; cleanup and argument widths \
                   come from verified HIR, clobbers from the calling convention, while \
                   memory remains conservative"
            .to_owned(),
        clobbers: runtime::EVERY.difference(preserved).copied().collect(),
        ..runtime::worst(name)
    })
}

/// The ABI of the MIR a HIR program emits: a runtime routine linked by its
/// own name and called by its contract, any other function by its
/// frontend's object name.
pub struct HirAbi {
    pub runtime: model::RuntimeProfile,
    /// Each function's object name, where it is not its HIR name.
    pub objects: std::collections::BTreeMap<String, String>,
    /// The registers a call no runtime contract describes keeps.
    pub preserved: BTreeSet<runtime::Reg>,
    /// The runtime's stack limit and overflow handler, where the program checks
    /// its stack.
    pub stack_check: Option<model::StackCheck>,
}

impl HirAbi {
    /// `program`'s calls: its runtime's, and its own by their symbols.
    pub fn of(program: &model::Program) -> Result<Self, String> {
        let functions = program.modules.iter().flat_map(|module| &module.functions);
        Ok(Self {
            runtime: program.runtime,
            objects: functions.filter_map(|one| Some((one.name.clone(), one.symbol.clone()?))).collect(),
            preserved: program.preserved.iter().map(|one| runtime::Reg::from_value(one)).collect::<Result<_, _>>()?,
            stack_check: program.stack_check.clone(),
        })
    }
}

/// A runtime routine's register interface beside its stack block. B$HARY
/// takes the subscripts and their count pushed and the descriptor in BX,
/// and answers the element's address in ES:BX: PDS71 PDHUGE.OBJ 0047..0055
/// sets BX last and uses ES:BX at once.
pub fn registers(name: &str) -> Option<Registers> {
    match name {
        "B$HARY" => Some(Registers { arguments: vec![Register::BX], results: vec![Register::BX, Register::ES] }),
        _ => None,
    }
}

impl crate::backend::assemble::Abi for HirAbi {
    fn stack_check(&self) -> Option<&model::StackCheck> {
        self.stack_check.as_ref()
    }

    fn registers(
        &self,
        callee: &str,
    ) -> Option<Registers> {
        if let Some(block) = asm_call(callee) {
            return block.ok().map(|(_, registers)| registers);
        }
        registers(callee.strip_prefix(crate::hir::mir::RUNTIME).unwrap_or(callee))
    }

    fn contract(
        &self,
        callee: &str,
        pops: bool,
        pushed: i64,
    ) -> Result<Contract, String> {
        if let Some(block) = asm_call(callee) {
            return block.map(|(contract, _)| contract).map_err(|error| error.0);
        }
        let cleanup = if pops { model::StackCleanup::Callee } else { model::StackCleanup::Caller };
        let name = callee.strip_prefix(crate::hir::mir::RUNTIME).unwrap_or(callee);
        _contract_keeping(name, cleanup, pushed, self.runtime, &self.preserved).map_err(|error| error.0)
    }

    fn linked(
        &self,
        name: &str,
    ) -> String {
        match name.strip_prefix(crate::hir::mir::RUNTIME) {
            Some(routine) => routine.to_owned(),
            None => self.objects.get(name).cloned().unwrap_or_else(|| name.to_owned()),
        }
    }
}

/// A CPU's target as the backend lowers to it: a call keeps the registers
/// its callee's contract leaves, and a multiply by a constant costs its
/// cheapest chain, as `abi` and `arithmetic` lower them.
pub struct LoweredTarget {
    machine: std::rc::Rc<dyn llrm_mir::target::Machine>,
    abi: HirAbi,
    cpu: &'static crate::backend::cpu::Profile,
}

impl LoweredTarget {
    pub fn of(
        cpu: &'static crate::backend::cpu::Profile,
        abi: HirAbi,
    ) -> Self {
        Self { machine: cpu.target(), abi, cpu }
    }
}

impl llrm_mir::target::Machine for LoweredTarget {
    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        self.machine.spaces()
    }

    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        self.machine.foreign_span(selectors, offsets, width)
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        self.machine.costs()
    }

    fn size_costs(&self) -> llrm_mir::target::OperationCosts {
        self.machine.size_costs()
    }

    fn registers(&self) -> i64 {
        self.machine.registers()
    }

    fn call_registers(&self) -> i64 {
        self.machine.call_registers()
    }

    fn far_access_registers(&self) -> i64 {
        self.machine.far_access_registers()
    }

    fn segment_registers(&self) -> i64 {
        self.machine.segment_registers()
    }

    fn two_address(&self) -> bool {
        self.machine.two_address()
    }

    fn private_convention(&self) -> Option<llrm_mir::target::PrivateConvention> {
        self.machine.private_convention()
    }

    fn callee_pop(
        &self,
        convention: u32,
    ) -> Option<u32> {
        self.machine.callee_pop(convention)
    }

    fn stack_argument_bytes(
        &self,
        convention: u32,
        arguments: &[llrm_mir::target::Argument],
    ) -> Option<i64> {
        self.machine.stack_argument_bytes(convention, arguments)
    }

    fn address_registers(&self) -> i64 {
        self.machine.address_registers()
    }

    fn kept_across(
        &self,
        callee: Option<&str>,
    ) -> i64 {
        use crate::backend::assemble::Abi;
        match callee.map(|name| self.abi.contract(name, false, 0)) {
            Some(Ok(contract)) => crate::backend::callregs::call_keeps(&contract).len() as i64,
            _ => self.machine.call_registers(),
        }
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        self.machine.address_forms()
    }

    fn huge_window(&self) -> Option<(u32, i64)> {
        self.machine.huge_window()
    }

    fn multiply_by(
        &self,
        factor: i64,
    ) -> i64 {
        use crate::backend::arithmetic;
        let multiply =
            arithmetic::immediate_multiply(self.cpu, factor).unwrap_or_else(|_| self.machine.costs().multiply);
        match arithmetic::cheapest_chain(factor, self.cpu) {
            Ok(Some((_, clocks))) => clocks.min(multiply),
            _ => multiply,
        }
    }

    fn load_may_trap(
        &self,
        width: u64,
        align: u64,
    ) -> bool {
        self.machine.load_may_trap(width, align)
    }

    fn port_touches_memory(
        &self,
        ports: (i64, i64),
    ) -> bool {
        self.machine.port_touches_memory(ports)
    }
}
