//! Port of `qbopt/frontend/qb/abi.py`: late QB-family call ABI
//! materialization.
//!
//! HIR and optimizing MIR keep typed arguments on the call.  Only after those
//! passes have finished do stack order and cleanup become physical ARG nodes.
//! This is deliberately frontend-owned: no QB convention is added to MIR or
//! the backend.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::LazyLock;

use iced_x86::Register;
use crate::support::hash::{IndexMap, IndexSet};

use crate::abi::runtime::{self, Contract, Control};
use crate::backend::pointers;
use crate::hir::lower::Lowered;
use crate::hir::model;
use crate::model::floating;
use crate::model::ir::Operation;
use crate::model::memory::{Identity, MemoryKind, MemoryObject, Provenance};
use crate::model::mir::{self, Arg, Kind, OpCode, Synth};
use crate::objectfile::module::{Addr, Space};
use crate::support::pyrepr::{self, Repr};

/// A typed call lacks enough measured ABI information to emit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbiError(pub String);

impl fmt::Display for AbiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
static _AUDITED_STRING_STACK: LazyLock<IndexMap<&str, i64>> = LazyLock::new(|| IndexMap::from_iter([
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
]));

// Stack widths read from actual QB 4.5 /A listings for the graphics forms,
// then checked against their runtime RETF cleanup. Coordinate state is latched
// by N1/N2 before the drawing call; it is deliberately not hidden in MIR.
static _AUDITED_GRAPHICS_STACK: LazyLock<IndexMap<&str, i64>> = LazyLock::new(|| IndexMap::from_iter([
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
]));

static _AUDITED_STATEMENT_STACK: LazyLock<IndexMap<&str, i64>> = LazyLock::new(|| {
    IndexMap::from_iter([
        ("B$BEEP", 0),
        ("B$LNIN", 10),
        // Path descriptor, channel, record length -1 and mode; BCOM45 dkopen.asm
        // B$OPEN at 0224 returns with RETF 8 at 0252.
        ("B$OPEN", 8),
        ("B$SLEP", 4),
    ])
});

#[derive(Clone, Debug)]
pub struct Physicalized {
    pub lowered: Lowered,
    pub calls: IndexMap<i64, String>,
    pub contracts: IndexMap<i64, Contract>,
    pub far_calls: BTreeSet<i64>,
    pub pointer_model: pointers::Model,
    pub hints: mir::AllocationHints,
    /// Every formal's entry cell, lowest address first: the order a caller's
    /// pushes bind them, last push first.
    pub parameters: Vec<mir::MemRef>,
    /// Each inline block, by the site of the call laying it down.
    pub inline: IndexMap<i64, InlineCode>,
}

/// An inline block's machine code and the registers its results come out of.
#[derive(Clone, Debug, PartialEq)]
pub struct InlineCode {
    pub code: Vec<u8>,
    pub outputs: Vec<Register>,
}

impl Physicalized {
    /// `hints` with each inline block's results where it leaves them. Keyed
    /// by site, not by value: a pass that copies a block gives the copy new
    /// values but the same site.
    pub fn hints_for(&self, body: &mir::MirBody) -> mir::AllocationHints {
        let mut hints = self.hints.clone();
        for op in body.blocks.iter().flat_map(|block| &block.ops).filter(|op| op.kind == Kind::Call) {
            let Some(inline) = self.inline.get(&op.at) else {
                continue;
            };
            for (result, register) in op.results.iter().zip(&inline.outputs) {
                if let Arg::Held(held) = result {
                    hints.origins.insert(held.value.variable, *register);
                }
            }
        }
        hints
    }
}

/// An inline block as a call: its declared registers are all it reads and changes.
fn _inline_contract(asm: &model::Asm) -> Result<(Contract, Vec<runtime::Reg>, Vec<runtime::Reg>), AbiError> {
    let registers = |names: &[String]| {
        names
            .iter()
            .map(|name| {
                crate::backend::inline_asm::named(name)
                    .ok_or_else(|| AbiError(format!("inline assembly names no register {}", pyrepr::string(name))))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let (inputs, outputs, clobbers) = (registers(&asm.inputs)?, registers(&asm.outputs)?, registers(&asm.clobbers)?);
    let memory = if asm.memory { runtime::Memory::Any } else { runtime::Memory::None };
    let contract = Contract {
        name: crate::hir::lower::ASM.to_owned(),
        cleanup: Some(0),
        control: Control::Returns,
        enters_user_code: false,
        raises_error: false,
        error_handling: false,
        writes: memory,
        reads: memory,
        clobbers: clobbers.iter().chain(&outputs).copied().collect(),
        established: true,
        evidence: "inline assembly: the inputs, outputs and clobbers it declares".to_owned(),
        documented: None,
        inputs: Some(inputs.iter().copied().collect()),
        direct_inputs: None,
        clobbers_reached: true,
        caller_cleanup: 0,
        // It names no 32-bit register, so it keeps every high half.
        i386: false,
        direct_writes: None,
        direct_reads: None,
    };
    Ok((contract, inputs, outputs))
}

/// An inline block's call, its arguments in the contract's slot order, each
/// a value: a constant is copied into one first.
#[allow(clippy::type_complexity)]
fn _inline_block(
    operation: &mir::Op,
    asm: &model::Asm,
    next_value: &mut u32,
    next_at: &mut i64,
) -> Result<(Vec<mir::Op>, mir::Op, Contract, InlineCode), AbiError> {
    let (contract, inputs, outputs) = _inline_contract(asm)?;
    let mut slotted: Vec<(runtime::Reg, &Arg)> = inputs.into_iter().zip(&operation.args).collect();
    slotted.sort_by_key(|(register, _)| runtime::SLOTS.iter().position(|slot| slot == register));
    let mut copies = Vec::new();
    let mut args = Vec::new();
    for (_, argument) in slotted {
        if let Arg::Held(_) = argument {
            args.push(argument.clone());
            continue;
        }
        let value = mir::Value { variable: *next_value, version: 1, ..mir::Value::new(*next_value, *next_at) };
        *next_value += 1;
        let held = mir::Held { value, width: 2 };
        let mut copy = mir::Op::new(*next_at, OpCode::Operation(Operation::Move), "mov", vec![value], vec![]);
        copy.kind = Kind::Copy;
        copy.args = vec![argument.clone()];
        copy.results = vec![Arg::Held(held)];
        copy.source = Some(*next_at as u32);
        copy.reads_complete = true;
        copy.memory_complete = true;
        copies.push(copy);
        *next_at += 1;
        args.push(Arg::Held(held));
    }
    let uses = args
        .iter()
        .filter_map(|one| match one {
            Arg::Held(held) => Some(held.value),
            _ => None,
        })
        .collect();
    let code = InlineCode {
        code: asm.code.iter().map(|byte| *byte as u8).collect(),
        outputs: outputs.into_iter().filter_map(mir::as_named).collect(),
    };
    Ok((copies, mir::Op { args, uses, ..operation.clone() }, contract, code))
}

fn part_width(argument: &Arg) -> u32 {
    match argument {
        Arg::Cell(cell) => cell.r#ref.width,
        Arg::Held(one) => one.width,
        _ => 2,
    }
}

/// A result that comes back in DX:AX, as C returns it: a 4-byte integer or
/// far pointer.
fn _paired(type_: &model::Type) -> bool {
    matches!(type_.kind, model::TypeKind::Integer | model::TypeKind::Pointer) && type_.width == 4
}

fn _bytes(argument: &Arg) -> Result<i64, AbiError> {
    let width = match argument {
        Arg::Cell(cell) => Some(cell.r#ref.width),
        Arg::Held(one) => Some(one.width),
        Arg::Const(one) => Some(one.width),
        Arg::Symbol(one) => Some(one.width),
        Arg::FrameAddress(one) => Some(one.width),
        Arg::FrameSelector(one) => Some(one.width),
        Arg::Opaque(_) => None,
    };
    match width {
        Some(width) if width > 0 => Ok(2.max(i64::from(width))),
        _ => Err(AbiError(format!("call argument {} has no physical width", argument.repr()))),
    }
}

/// Keep a call's memory identity without inventing an encoded address.
///
/// Physical ABI lowering has already emitted pointer arguments as stack
/// writes, so the call itself reads no address value in a register; leaving
/// them made two far selectors compete for ES at B$ASSN.
fn _stack_effect(reference: &mir::MemRef) -> mir::MemRef {
    mir::MemRef { base: None, segment: None, ..reference.clone() }
}

/// Spell a stored DOUBLE as two legal 386 dword pushes, high first.
///
/// QB's Pascal stack presents the low byte at `[bp+6]`, so a multiword value
/// is pushed from its high end toward its low end.  An unlocated 8-byte value
/// is rejected: a hidden split would lose its rounding boundary and order.
fn _stack_argument_parts(argument: &Arg) -> Result<Vec<Arg>, AbiError> {
    let Arg::Cell(cell) = argument else {
        return Ok(vec![argument.clone()]);
    };
    if cell.r#ref.width != 8 {
        return Ok(vec![argument.clone()]);
    }
    let Some(addr) = cell.r#ref.addr else {
        return Err(AbiError("8-byte stack argument needs addressable declared-width storage".into()));
    };
    Ok([4, 0]
        .into_iter()
        .map(|offset| {
            Arg::Cell(mir::Cell {
                r#ref: mir::MemRef {
                    addr: Some(addr.plus(offset)),
                    width: 4,
                    provenance: cell.r#ref.provenance.as_ref().map(|one| one.shifted(offset)),
                    ..cell.r#ref.clone()
                },
            })
        })
        .collect())
}

pub fn _contract(
    name: &str,
    cleanup: model::StackCleanup,
    pushed: i64,
    family: model::RuntimeProfile,
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
    let found = runtime::per_call(&IndexMap::from_iter([(0, physical_name.to_owned())]), family.value(), &BTreeSet::new())
        .swap_remove(&0)
        .expect("one call in, one contract out");
    let evidence = &found.evidence;
    // `replace(found, cleanup=..., control=..., enters_user_code=...,
    // established=True, inputs=frozenset(), i386=True, evidence=...)`.
    let refined = |found: &Contract, cleanup: i64, control: Control, enters_user_code: bool, evidence: String| {
        Contract {
            cleanup: Some(cleanup),
            control,
            enters_user_code,
            established: true,
            inputs: Some(BTreeSet::new()),
            i386: true,
            evidence,
            ..found.clone()
        }
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
        // however, the wrapper's own RETF 4/8 and exact argument width establish
        // normal cleanup without making any stronger effect claim.
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
                   come from verified HIR, while memory and clobbers remain conservative"
            .to_owned(),
        ..runtime::worst(name)
    })
}

/// Turn one optimized semantic body into the backend's existing call form.
pub fn physicalize(
    program: &model::Program,
    function: &model::Function,
    lowered: &Lowered,
) -> Result<Physicalized, AbiError> {
    let _held = |value: mir::Value, width: u32| mir::Held { value, width };
    let parameter_object = |identity: Identity, extent: Option<i64>| MemoryObject {
        identity: Some(identity),
        extent,
        ..MemoryObject::new(MemoryKind::Parameter)
    };
    let sites: IndexMap<i64, &model::CallAbi> = function.calls.iter().map(|one| (one.instruction, one)).collect();
    let mut next_at = lowered.body.blocks.iter().flat_map(|block| &block.ops).map(|op| op.at).max().unwrap_or(0) + 1;
    let mut calls: IndexMap<i64, String> = IndexMap::default();
    let mut contracts: IndexMap<i64, Contract> = IndexMap::default();
    let mut far: BTreeSet<i64> = BTreeSet::new();
    let module = program
        .modules
        .iter()
        .find(|one| one.functions.contains(function))
        .expect("the function belongs to this program");
    let callable_names: IndexMap<i64, &str> = module.callables.iter().map(|one| (one.id, one.name.as_str())).collect();
    let callables: IndexMap<i64, &model::Callable> = module.callables.iter().map(|one| (one.id, one)).collect();
    let types: IndexMap<i64, &model::Type> = module.types.iter().map(|one| (one.id, one)).collect();
    let value_types: IndexMap<mir::Value, &model::Type> = function
        .values
        .iter()
        .filter(|value| lowered.values.contains_key(&value.id))
        .map(|value| (lowered.values[&value.id], types[&value.r#type]))
        .collect();

    let site_of = |operation: &mir::Op| operation.source.and_then(|id| sites.get(&i64::from(id)).copied());
    let blocks_of: IndexMap<i64, &model::Asm> = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.asm.as_ref().map(|asm| (instruction.id, asm)))
        .collect();
    let mut inline: IndexMap<i64, InlineCode> = IndexMap::default();
    // Below every frame place: where a float argument waits to be pushed.
    let float_argument_at = function
        .places
        .iter()
        .filter(|place| matches!(place.storage, model::Storage::Local | model::Storage::Parameter))
        .map(|place| place.offset)
        .min()
        .unwrap_or(0)
        .min(0)
        - 8;
    let call_name = |operation: &mir::Op| -> Result<String, AbiError> {
        if operation.source.is_some_and(|id| blocks_of.contains_key(&i64::from(id))) {
            return Ok(operation.name.clone());
        }
        let Some(site) = site_of(operation) else {
            return Err(AbiError(format!("call {} has no ABI site", operation.source.repr())));
        };
        Ok(match site.callee {
            Some(callee) => callable_names[&callee].to_owned(),
            None => operation.name.clone(),
        })
    };

    let mut hary_results: IndexSet<(mir::Value, mir::Value)> = IndexSet::default();
    for source_block in &lowered.body.blocks {
        for source_operation in &source_block.ops {
            if source_operation.kind != Kind::Call || call_name(source_operation)? != "B$HARY" {
                continue;
            }
            if source_operation.results.len() != 2
                || source_operation.results.iter().any(|one| !matches!(one, Arg::Held(held) if held.width == 2))
            {
                return Err(AbiError("B$HARY needs explicit INTEGER offset and selector results".into()));
            }
            let (Arg::Held(offset), Arg::Held(selector)) = (&source_operation.results[0], &source_operation.results[1])
            else {
                unreachable!()
            };
            hary_results.insert((offset.value, selector.value));
        }
    }
    let mut hary_pointers: IndexMap<mir::Value, (mir::Value, mir::Value)> = IndexMap::default();
    for source_block in &lowered.body.blocks {
        for source_operation in &source_block.ops {
            if source_operation.kind == Kind::Concat
                && source_operation.args.len() == 2
                && source_operation.results.len() == 1
            {
                if let (Arg::Held(high), Arg::Held(low), Arg::Held(result)) =
                    (&source_operation.args[0], &source_operation.args[1], &source_operation.results[0])
                {
                    if hary_results.contains(&(low.value, high.value)) {
                        hary_pointers.insert(result.value, (low.value, high.value));
                    }
                }
            }
        }
    }

    let hary_reference = |reference: &mir::MemRef| -> Result<mir::MemRef, AbiError> {
        let Some(parts) = reference.base.and_then(|base| hary_pointers.get(&base)) else {
            return Ok(reference.clone());
        };
        if !reference.pointer || reference.base_width != 4 || reference.addr.is_some() || reference.segment.is_some() {
            return Err(AbiError("B$HARY result escaped as something other than one element address".into()));
        }
        let (offset, selector) = *parts;
        Ok(mir::MemRef {
            addr: Some(Addr::new(Space::Far, 0)),
            base: Some(offset),
            segment: Some(selector),
            space: Some(Space::Far),
            base_width: 2,
            pointer: false,
            ..reference.clone()
        })
    };

    let hary_access = |operation: &mir::Op| -> Result<mir::Op, AbiError> {
        let replaced: IndexMap<mir::Value, (mir::Value, mir::Value)> = hary_pointers
            .iter()
            .filter(|(value, _)| operation.uses.contains(value))
            .map(|(value, parts)| (*value, *parts))
            .collect();
        if replaced.is_empty() {
            return Ok(operation.clone());
        }
        let argument = |one: &Arg| -> Result<Arg, AbiError> {
            Ok(match one {
                Arg::Cell(cell) => Arg::Cell(mir::Cell { r#ref: hary_reference(&cell.r#ref)? }),
                other => other.clone(),
            })
        };
        let mut uses: IndexSet<mir::Value> = IndexSet::default();
        for value in &operation.uses {
            match replaced.get(value) {
                Some((offset, selector)) => {
                    uses.insert(*offset);
                    uses.insert(*selector);
                }
                None => {
                    uses.insert(*value);
                }
            }
        }
        Ok(mir::Op {
            uses: uses.into_iter().collect(),
            args: operation.args.iter().map(argument).collect::<Result<_, _>>()?,
            results: operation.results.iter().map(argument).collect::<Result<_, _>>()?,
            loads: operation.loads.iter().map(hary_reference).collect::<Result<_, _>>()?,
            stores: operation.stores.iter().map(hary_reference).collect::<Result<_, _>>()?,
            ..operation.clone()
        })
    };

    // A fresh value exceeds every one the body names: a formal is used, never defined.
    let used = lowered.body.blocks.iter().flat_map(|block| block.ops.iter().flat_map(|op| op.uses.iter().copied()));
    let mut next_value = lowered.body.values().into_iter().chain(used).map(|value| value.id).max().unwrap_or(0) + 1;
    let mut origins: mir::OrderedMap<u32, Register> = mir::OrderedMap::new();

    let fresh_value = |next_value: &mut u32, next_at: i64| -> mir::Value {
        let value = mir::Value { variable: *next_value, version: 1, ..mir::Value::new(*next_value, next_at) };
        *next_value += 1;
        value
    };

    let long_halves = |source: &mir::Held, next_value: &mut u32, next_at: &mut i64| {
        let mut halves = Vec::new();
        let mut made = Vec::new();
        for offset in [0, 16] {
            let value = fresh_value(next_value, *next_at);
            let held = _held(value, 2);
            let mut op = mir::Op::new(*next_at, OpCode::Synth(Synth::HalfToLow), "extract", vec![value], vec![source.value]);
            op.kind = Kind::Extract;
            op.args = vec![Arg::Held(*source), Arg::Const(mir::Const::new(offset, 1))];
            op.results = vec![Arg::Held(held)];
            op.source = Some(*next_at as u32);
            op.reads_complete = true;
            op.memory_complete = true;
            halves.push(op);
            made.push(held);
            *next_at += 1;
        }
        (halves, made[0], made[1])
    };

    let result_type = types[&function.result_type];
    let returns_legacy_long = _paired(result_type);
    let mut entry_loads = Vec::new();
    let mut formals: Vec<(i64, mir::MemRef)> = Vec::new();
    let parameter_types: Vec<&model::Type> = function
        .parameters
        .iter()
        .map(|parameter| types[&function.values.iter().find(|one| one.id == *parameter).expect("a parameter value").r#type])
        .collect();
    let callee_cleanup = function.abi.as_ref().is_some_and(|abi| abi.cleanup == model::StackCleanup::Callee);
    // BASIC's own function convention, where the ABI says so.
    let returns_legacy_float = result_type.kind == model::TypeKind::Float
        && function.abi.as_ref().is_some_and(|abi| abi.float_return == model::FloatReturn::Pointer)
        && callee_cleanup
        && !function.parameters.is_empty()
        && parameter_types[parameter_types.len() - 1].kind == model::TypeKind::Pointer
        && parameter_types[parameter_types.len() - 1].element == Some(result_type.id);
    let hidden_float_result =
        if returns_legacy_float { Some(lowered.values[&function.parameters[function.parameters.len() - 1]]) } else { None };
    let parameter_widths: Vec<i64> = parameter_types.iter().map(|type_| 2.max(type_.width)).collect();
    // A Pascal BASIC caller evaluates and pushes left-to-right, so the first
    // source formal is furthest from the return address. CDECL pushes
    // right-to-left and therefore retains the ordinary ascending layout.
    // Above BP: the saved BP and a near or far return address.
    let near = function.abi.as_ref().is_some_and(|abi| abi.distance == model::CallDistance::Near);
    let first = if near { 4 } else { 6 };
    let mut parameter_offsets = Vec::new();
    if callee_cleanup {
        let mut cursor = first + parameter_widths.iter().sum::<i64>();
        for width in &parameter_widths {
            cursor -= width;
            parameter_offsets.push(cursor);
        }
    } else {
        let mut cursor = first;
        for width in &parameter_widths {
            parameter_offsets.push(cursor);
            cursor += width;
        }
    }
    let read: BTreeSet<mir::Value> = lowered
        .body
        .blocks
        .iter()
        .flat_map(|block| {
            block.ops.iter().flat_map(|op| op.uses.iter().copied()).chain(block.phis.iter().flat_map(|phi| phi.incoming.values().copied()))
        })
        .collect();
    for (number, ((parameter, type_), parameter_offset)) in
        function.parameters.iter().zip(&parameter_types).zip(&parameter_offsets).enumerate()
    {
        let value = lowered.values[parameter];
        let object_ = parameter_object(Identity::Int(number as i64), Some(type_.width));
        let reference = mir::MemRef {
            space: Some(Space::Frame),
            provenance: Some(
                Provenance::one_with_slice(object_, 0, type_.width, 1, 1, BTreeSet::new())
                    .map_err(|error| AbiError(error.to_string()))?,
            ),
            ..mir::MemRef::new(Some(Addr::new(Space::Frame, *parameter_offset)), type_.width as u32)
        };
        formals.push((*parameter_offset, reference.clone()));
        // The load is the ABI's, not the program's: a parameter nothing reads
        // is not loaded, so a float one adds no observable operation.
        if !read.contains(&value) && hidden_float_result != Some(value) {
            continue;
        }
        let result = _held(value, if type_.kind == model::TypeKind::Float { 10 } else { type_.width as u32 });
        let mut kind = Kind::Load;
        let mut operation = Operation::Move;
        let mut name = "mov";
        let mut semantics = None;
        if type_.kind == model::TypeKind::Float {
            kind = Kind::Fload;
            operation = Operation::FloatLoad;
            name = "fld";
            let stored = if type_.width == 4 { floating::Format::Binary32 } else { floating::Format::Binary64 };
            semantics = Some(floating::Semantics::new(
                vec![stored],
                floating::Format::Extended80,
                floating::Precision::Exact,
                floating::Rounding::None,
            ));
        }
        let mut op = mir::Op::new(next_at, OpCode::Operation(operation), name, vec![value], vec![]);
        op.floating = semantics;
        op.loads = vec![reference.clone()];
        op.kind = kind;
        op.args = vec![Arg::Cell(mir::Cell { r#ref: reference })];
        op.results = vec![Arg::Held(result)];
        op.source = Some(next_at as u32);
        op.reads_complete = true;
        op.memory_complete = true;
        entry_loads.push(op);
        next_at += 1;
    }
    let mut blocks = Vec::new();
    for block in &lowered.body.blocks {
        let mut operations: Vec<mir::Op> = Vec::new();
        for source_operation in &block.ops {
            if source_operation.kind == Kind::Concat
                && source_operation.results.len() == 1
                && matches!(&source_operation.results[0], Arg::Held(held) if hary_pointers.contains_key(&held.value))
            {
                continue;
            }
            let operation = hary_access(source_operation)?;
            if operation.kind == Kind::Return && returns_legacy_float && operation.args.len() == 1 {
                let source = &operation.args[0];
                let (Arg::Held(source), Some(hidden_float_result)) = (source, hidden_float_result) else {
                    return Err(AbiError(format!("{} floating return is not one materialized value", function.name)));
                };
                if source.width != 10 {
                    return Err(AbiError(format!("{} floating return is not one materialized value", function.name)));
                }
                let pointer = _held(hidden_float_result, 2);
                let object_ = parameter_object(
                    Identity::Tuple(vec![Identity::Int(function.id), Identity::Str("float-result".into())]),
                    None,
                );
                let reference = mir::MemRef {
                    base: Some(hidden_float_result),
                    space: Some(Space::Literal),
                    base_width: 2,
                    provenance: Some(Provenance::one(object_)),
                    ..mir::MemRef::new(Some(Addr::new(Space::Literal, 0)), result_type.width as u32)
                };
                let stored = floating::Semantics::new(
                    vec![floating::Format::Extended80],
                    if result_type.width == 4 { floating::Format::Binary32 } else { floating::Format::Binary64 },
                    floating::Precision::Destination,
                    floating::Rounding::Dynamic,
                );
                let mut op = mir::Op::new(
                    next_at,
                    OpCode::Operation(Operation::FloatStore),
                    "fstp",
                    vec![],
                    vec![source.value, hidden_float_result],
                );
                op.floating = Some(stored);
                op.stores = vec![reference.clone()];
                op.kind = Kind::Fstore;
                op.args = vec![Arg::Held(*source)];
                op.results = vec![Arg::Cell(mir::Cell { r#ref: reference })];
                op.source = Some(next_at as u32);
                op.reads_complete = true;
                op.memory_complete = true;
                operations.push(op);
                next_at += 1;
                operations.push(mir::Op {
                    args: vec![Arg::Held(pointer)],
                    uses: vec![hidden_float_result],
                    ..operation.clone()
                });
                continue;
            }
            if operation.kind == Kind::Return && returns_legacy_long && operation.args.len() == 1 {
                let Arg::Held(source) = &operation.args[0] else {
                    return Err(AbiError(format!("{} LONG return is not one materialized dword", function.name)));
                };
                if source.width != 4 {
                    return Err(AbiError(format!("{} LONG return is not one materialized dword", function.name)));
                }
                let (halves, low, high) = long_halves(source, &mut next_value, &mut next_at);
                operations.extend(halves);
                operations.push(mir::Op {
                    args: vec![Arg::Held(low), Arg::Held(high)],
                    uses: vec![low.value, high.value],
                    ..operation.clone()
                });
                continue;
            }
            if operation.kind != Kind::Call {
                operations.push(operation);
                continue;
            }
            if let Some(asm) = operation.source.and_then(|id| blocks_of.get(&i64::from(id))) {
                let (copies, call, contract, code) = _inline_block(&operation, asm, &mut next_value, &mut next_at)?;
                operations.extend(copies);
                calls.insert(call.at, call.name.clone());
                contracts.insert(call.at, contract);
                inline.insert(call.at, code);
                operations.push(call);
                continue;
            }
            let Some(site) = site_of(&operation) else {
                return Err(AbiError(format!("call {} has no ABI site", operation.source.repr())));
            };
            let name = call_name(&operation)?;
            let ordered: Vec<Arg> = site.order.iter().map(|index| operation.args[*index as usize].clone()).collect();
            let mut fixed_arguments: Vec<Arg> = Vec::new();
            let mut stack_arguments = ordered.clone();
            if name == "B$HARY" {
                if ordered.len() < 3 || _bytes(&ordered[ordered.len() - 1])? != 2 {
                    return Err(AbiError("B$HARY needs subscripts, rank, and one near descriptor".into()));
                }
                stack_arguments = ordered[..ordered.len() - 1].to_vec();
                fixed_arguments = ordered[ordered.len() - 1..].to_vec();
            }
            // A float is held extended; the callee's parameter type says the
            // format it is passed in.
            let passed: Vec<Option<floating::Format>> = site
                .order
                .iter()
                .map(|index| {
                    let callee = callables.get(&site.callee?)?;
                    let type_ = types[callee.parameter_types.get(*index as usize)?];
                    (type_.kind == model::TypeKind::Float)
                        .then_some(if type_.width == 4 { floating::Format::Binary32 } else { floating::Format::Binary64 })
                })
                .collect();
            // x87 cannot push: a float is stored in that format to the frame
            // cell below all others, and the cell is what is pushed.
            for (number, argument) in stack_arguments.iter_mut().enumerate() {
                let (Arg::Held(source), Some(format)) = (&*argument, passed.get(number).copied().flatten()) else {
                    continue;
                };
                if source.width != 10 {
                    continue;
                }
                let width = if format == floating::Format::Binary32 { 4 } else { 8 };
                let object_ = MemoryObject {
                    identity: Some(Identity::Tuple(vec![Identity::Int(function.id), Identity::Str("float-argument".into())])),
                    extent: Some(8),
                    ..MemoryObject::new(MemoryKind::Frame)
                };
                let reference = mir::MemRef {
                    space: Some(Space::Frame),
                    provenance: Some(
                        Provenance::one_with_slice(object_, 0, width, 1, 1, BTreeSet::new())
                            .map_err(|error| AbiError(error.to_string()))?,
                    ),
                    ..mir::MemRef::new(Some(Addr::new(Space::Frame, float_argument_at)), width as u32)
                };
                let mut op = mir::Op::new(next_at, OpCode::Operation(Operation::FloatStore), "fstp", vec![], vec![source.value]);
                op.floating = Some(floating::Semantics::new(
                    vec![floating::Format::Extended80],
                    format,
                    floating::Precision::Destination,
                    floating::Rounding::Dynamic,
                ));
                op.stores = vec![reference.clone()];
                op.kind = Kind::Fstore;
                op.args = vec![argument.clone()];
                op.results = vec![Arg::Cell(mir::Cell { r#ref: reference.clone() })];
                op.source = Some(next_at as u32);
                op.reads_complete = true;
                op.memory_complete = true;
                operations.push(op);
                next_at += 1;
                *argument = Arg::Cell(mir::Cell { r#ref: reference });
            }
            let mut pushed = 0;
            for one in &stack_arguments {
                pushed += _bytes(one)?;
            }
            for argument in &stack_arguments {
                for part in _stack_argument_parts(argument)? {
                    let part = match part {
                        // A push moves a word; a byte argument is zero-extended to one
                        // first, and the callee reads its low byte at the same offset.
                        Arg::Const(one) if one.width == 1 => {
                            Arg::Const(mir::Const { width: 2, ..one })
                        }
                        Arg::Held(_) | Arg::Cell(_)
                            if _bytes(&part)? > i64::from(part_width(&part)) =>
                        {
                            let value = fresh_value(&mut next_value, next_at);
                            let uses = match &part {
                                Arg::Held(held) => vec![held.value],
                                Arg::Cell(cell) => cell.r#ref.base.into_iter().collect(),
                                _ => unreachable!("a held or memory argument"),
                            };
                            let mut widen = mir::Op::new(
                                next_at,
                                OpCode::Operation(Operation::Extend),
                                "movzx",
                                vec![value],
                                uses,
                            );
                            widen.kind = Kind::ZeroExtend;
                            if let Arg::Cell(cell) = &part {
                                widen.loads = vec![cell.r#ref.clone()];
                            }
                            widen.args = vec![part];
                            widen.results = vec![Arg::Held(_held(value, 2))];
                            widen.source = Some(next_at as u32);
                            widen.reads_complete = true;
                            widen.memory_complete = true;
                            operations.push(widen);
                            next_at += 1;
                            Arg::Held(_held(value, 2))
                        }
                        other => other,
                    };
                    let mut uses = match &part {
                        Arg::Held(held) => vec![held.value],
                        _ => vec![],
                    };
                    if let Arg::Cell(cell) = &part {
                        if let Some(base) = cell.r#ref.base {
                            uses = vec![base];
                        }
                    }
                    let mut op = mir::Op::new(next_at, OpCode::Operation(Operation::Push), "push", vec![], uses);
                    op.kind = Kind::Arg;
                    op.args = vec![part];

                    op.source = Some(next_at as u32);
                    op.reads_complete = true;
                    operations.push(op);
                    next_at += 1;
                }
            }
            calls.insert(operation.at, name.clone());
            contracts.insert(operation.at, _contract(&name, site.cleanup, pushed, program.runtime)?);
            if site.distance == model::CallDistance::Far {
                far.insert(operation.at);
            }
            let operation = mir::Op {
                loads: operation.loads.iter().map(_stack_effect).collect(),
                stores: operation.stores.iter().map(_stack_effect).collect(),
                ..operation.clone()
            };
            let result = if operation.results.len() == 1 { Some(operation.results[0].clone()) } else { None };
            let callable_ = site.callee.and_then(|callee| callables.get(&callee).copied());
            let semantic_type: Option<&model::Type> = match callable_ {
                Some(callable_) if callable_.result_type.is_some() => Some(types[&callable_.result_type.unwrap()]),
                _ => match &result {
                    Some(Arg::Held(held)) => value_types.get(&held.value).copied(),
                    _ => None,
                },
            };
            let legacy_long = matches!(&result, Some(Arg::Held(held)) if held.width == 4)
                && semantic_type.is_some_and(_paired);
            let legacy_float = matches!(&result, Some(Arg::Held(held)) if held.width == 10)
                && callable_.is_some()
                && semantic_type.is_some_and(|type_| type_.kind == model::TypeKind::Float)
                && site.cleanup == model::StackCleanup::Callee
                && site.float_return == model::FloatReturn::Pointer;
            if name == "B$HARY" {
                if operation.results.len() != 2
                    || operation.results.iter().any(|one| !matches!(one, Arg::Held(held) if held.width == 2))
                {
                    return Err(AbiError("B$HARY needs explicit INTEGER offset and selector results".into()));
                }
                let (Arg::Held(offset), Arg::Held(selector)) = (&operation.results[0], &operation.results[1]) else {
                    unreachable!()
                };
                origins.insert(offset.value.variable, Register::BX);
                origins.insert(selector.value.variable, Register::ES);
                let uses = fixed_arguments
                    .iter()
                    .filter_map(|one| match one {
                        Arg::Held(held) => Some(held.value),
                        _ => None,
                    })
                    .collect();
                operations.push(mir::Op { args: fixed_arguments, uses, ..operation });
                continue;
            }
            if legacy_float {
                let semantic_type = semantic_type.expect("legacy_float has a type");
                let Some(Arg::Held(result)) = result else { unreachable!() };
                let pointer_value = fresh_value(&mut next_value, next_at);
                let pointer = _held(pointer_value, 2);
                operations.push(mir::Op {
                    defines: vec![pointer_value],
                    results: vec![Arg::Held(pointer)],
                    args: vec![],
                    uses: vec![],
                    ..operation.clone()
                });
                let object_ = parameter_object(
                    Identity::Tuple(vec![
                        Identity::Int(i64::from(operation.source.expect("a call site has an id"))),
                        Identity::Str("float-result".into()),
                    ]),
                    None,
                );
                let reference = mir::MemRef {
                    base: Some(pointer_value),
                    space: Some(Space::Literal),
                    base_width: 2,
                    provenance: Some(Provenance::one(object_)),
                    ..mir::MemRef::new(Some(Addr::new(Space::Literal, 0)), semantic_type.width as u32)
                };
                let loaded = floating::Semantics::new(
                    vec![if semantic_type.width == 4 { floating::Format::Binary32 } else { floating::Format::Binary64 }],
                    floating::Format::Extended80,
                    floating::Precision::Exact,
                    floating::Rounding::None,
                );
                let mut op = mir::Op::new(
                    next_at,
                    OpCode::Operation(Operation::FloatLoad),
                    "fld",
                    vec![result.value],
                    vec![pointer_value],
                );
                op.floating = Some(loaded);
                op.loads = vec![reference.clone()];
                op.kind = Kind::Fload;
                op.args = vec![Arg::Cell(mir::Cell { r#ref: reference })];
                op.results = vec![Arg::Held(result)];
                op.source = Some(next_at as u32);
                op.reads_complete = true;
                op.memory_complete = true;
                operations.push(op);
                next_at += 1;
                continue;
            }
            if !legacy_long {
                operations.push(mir::Op { args: vec![], uses: vec![], ..operation });
                continue;
            }
            let Some(Arg::Held(result)) = result else { unreachable!() };
            let low = fresh_value(&mut next_value, next_at);
            let high = fresh_value(&mut next_value, next_at);
            operations.push(mir::Op {
                defines: vec![low, high],
                results: vec![Arg::Held(_held(low, 2)), Arg::Held(_held(high, 2))],
                args: vec![],
                uses: vec![],
                ..operation
            });
            let mut op = mir::Op::new(next_at, OpCode::Operation(Operation::Move), "", vec![result.value], vec![high, low]);
            op.kind = Kind::Concat;
            op.args = vec![Arg::Held(_held(high, 2)), Arg::Held(_held(low, 2))];
            op.results = vec![Arg::Held(result)];
            op.source = Some(next_at as u32);
            op.reads_complete = true;
            op.memory_complete = true;
            operations.push(op);
            next_at += 1;
        }
        if block.at == lowered.body.entry {
            operations = entry_loads.iter().cloned().chain(operations).collect();
        }
        blocks.push(block.with_ops(operations));
    }
    // HIR represents a source-level terminal statement with an explicit
    // UNREACHABLE terminator. Once this frontend's audited call contract says
    // the preceding runtime call never returns, that marker has no machine
    // operation and no remaining control-flow meaning. Keep it out of LIR;
    // emitting a placeholder after B$CEND/B$RESA also makes later block
    // placement mistake data-less syntax for a real instruction.
    let blocks: Vec<mir::MirBlock> = blocks
        .into_iter()
        .map(|block| {
            let count = block.ops.len();
            if count >= 2
                && block.ops[count - 1].kind == Kind::Escape
                && block.ops[count - 2].kind == Kind::Call
                && contracts.get(&block.ops[count - 2].at).is_some_and(|one| one.control == Control::Never)
            {
                let mut ops = block.ops.clone();
                ops.pop();
                mir::MirBlock { ops, ..block }
            } else {
                block
            }
        })
        .collect();
    let body = lowered.body.with_blocks(blocks);
    let mut checked = body.clone();
    let mut external: IndexSet<i64> = IndexSet::default();
    external.insert(body.entry);
    external.extend(function.external_entries.iter().copied());
    external.extend(function.error_handler);
    if external.len() > 1 {
        let root = body.blocks.iter().map(|block| block.at).max().expect("a body has blocks") + 1;
        checked = mir::MirBody { entry: root, ..body.with_blocks(std::iter::once(mir::MirBlock::new(root, vec![], vec![], external.into_iter().collect()))
                .chain(body.blocks.iter().cloned())
                .collect()) };
    }
    let problems = mir::verify(&checked);
    if !problems.is_empty() {
        return Err(AbiError(format!(
            "physicalized {} is invalid: {}",
            lowered.name,
            pyrepr::list(&problems[..problems.len().min(3)])
        )));
    }
    // DOS huge pointers advance their selector by 1 << 12 per wrapped 64 KiB
    // page (runtime nhinit.asm). Far accesses use the same established model
    // even when no arithmetic crosses a page.
    Ok(Physicalized {
        lowered: Lowered { body, ..lowered.clone() },
        calls,
        contracts,
        far_calls: far,
        pointer_model: pointers::Model::new(pointers::HugeShift::Fixed(12)).expect("12 is a valid shift"),
        hints: mir::AllocationHints { origins, ..mir::AllocationHints::new() },
        parameters: {
            formals.sort_by_key(|(offset, _)| *offset);
            formals.into_iter().map(|(_, reference)| reference).collect()
        },
        inline,
    })
}
