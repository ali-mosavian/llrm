//! `instructions/opcodes.instr` against the producer: the rows the shared
//! passes read must exist for every form the producer makes, and say what it
//! makes. A form with no row reads as "no effects known" (an empty `rows()`),
//! which is how arm64 ran every shared phase blind before the file.

use super::tests::{function, instructions};
use super::*;

/// Forms the round-trip fixture makes that AArch64 has no instruction for: it
/// crosses closed sets (every conversion width with every other, every `fmov`
/// register pair), and the selector never makes these. A call, a tail call and
/// a return carry a variable number of registers and have no row.
const NOT_INSTRUCTIONS: &[&str] = &[
    "FCVTDDr",
    "FCVTHHr",
    "FCVTSSr", // a conversion to the same size
    "FMOVDWr",
    "FMOVSXr",
    "FMOVWDr",
    "FMOVXSr", // `fmov` pairs of unequal size
    "RET_ReallyLR",
];

fn dialect() -> llrm_target::instr::Dialect {
    llrm_target::instr::Dialect::parse(include_str!("../instructions/dialect.toml")).expect("dialect.toml")
}

fn opcodes() -> Vec<llrm_target::instr::Opcode> {
    llrm_target::instr::parse_opcodes(include_str!("../instructions/opcodes.instr"), &dialect()).expect("opcodes.instr")
}

/// The semantics of every form the fixture makes, with the registers' classes
/// the instruction has (floating-point operands counted).
fn fixture() -> Vec<(Semantics, usize)> {
    let ops: Vec<Op> = instructions().into_iter().map(Op::I).collect();
    let lowered = to_lir(&function(ops.clone())).expect("every form maps");
    let floats: Vec<usize> = ops
        .iter()
        .map(|op| {
            let (defs, uses) = op.defs_and_uses();
            defs.iter().chain(&uses).filter(|one| matches!(one, Reg::Fpr(_))).count()
        })
        .collect();
    lowered.body.iter_insns().filter_map(|insn| insn.what.clone()).zip(floats).collect()
}

/// A form with no row is one the shared passes know nothing of: 8 of the
/// fixture's 418 names are not AArch64 instructions, every other has one.
#[test]
fn every_form_the_producer_makes_has_a_row() {
    let missing: Vec<String> = fixture()
        .iter()
        .filter(|(what, _)| {
            let name = what.name.as_deref().unwrap_or("");
            !NOT_INSTRUCTIONS.contains(&name)
                && (crate::REGISTER_INFO.effects)(name, what.dests.len(), what.sources.len()).is_empty()
        })
        .map(|(what, _)| format!("{} {}/{}", what.name.as_deref().unwrap_or(""), what.dests.len(), what.sources.len()))
        .collect();
    assert!(missing.is_empty(), "{} forms have no row: {missing:?}", missing.len());
}

/// A row's operands are the producer's: an immediate, a memory cell or an
/// address where the row says so, and as many floating-point registers.
#[test]
fn a_row_says_what_the_producer_makes() {
    let mut wrong = Vec::new();
    for (what, floats) in fixture() {
        let name = what.name.as_deref().unwrap_or("");
        let rows = (crate::REGISTER_INFO.effects)(name, what.dests.len(), what.sources.len());
        let Some(row) = rows.first() else { continue };
        let kinds: Vec<char> = what
            .dests
            .iter()
            .chain(&what.sources)
            .map(|place| match place {
                Loc::Imm(_) => 'i',
                Loc::Mem(_) => 'm',
                Loc::Address(_) => 'a',
                _ => 'r',
            })
            .collect();
        let said: Vec<char> =
            row.kinds.iter().map(|kind| if *kind == "x" { 'r' } else { kind.chars().next().unwrap_or('?') }).collect();
        let said_floats = row.kinds.iter().filter(|kind| **kind == "x").count();
        if kinds != said || said_floats != floats {
            wrong.push(format!("{name}: producer {kinds:?} with {floats} float, row {said:?} with {said_floats}"));
        }
    }
    assert!(wrong.is_empty(), "{} rows disagree: {wrong:#?}", wrong.len());
}

/// Every opcode in the file is stated, and a name with no addressing-mode or
/// size suffix is LLVM's own, or is one of the producer's few other spellings.
#[test]
fn a_name_without_a_suffix_is_llvms_or_is_listed() {
    // The producer's own spellings of a pseudo form, an alias or an opcode with
    // a different operand shape: each with the opcode LLVM calls it.
    const SPELLED: &[(&str, &str)] = &[
        ("ADDXri.lo12", "ADDXri"),
        ("ADDXri.fi", "ADDXri"),
        ("ADRP.got", "ADRP"),
        ("COPY_FPR", "COPY"),
        ("MOVImm", "MOVi64imm"),
        ("NOP", "HINT"),
        ("PRFM", "PRFMui"),
        ("SMADDLXrrr", "SMADDLrrr"),
        ("SMSUBLXrrr", "SMSUBLrrr"),
        ("UMADDLXrrr", "UMADDLrrr"),
        ("UMSUBLXrrr", "UMSUBLrrr"),
        ("SMULHXrrr", "SMULHrr"),
        ("UMULHXrrr", "UMULHrr"),
        ("DUPv16b", "DUPv16i8gpr"),
        ("MOVIv16b", "MOVIv16b_ns"),
        ("MOVIv2d", "MOVIv2d_ns"),
        ("ORRv16b", "ORRv16i8"),
        ("TBZ", "TBZ"),
        ("TBNZ", "TBNZ"),
    ];
    const FAMILIES: &[&str] = &["FCMP", "FCVTZ", "SCVTF", "UCVTF", "TB"];
    let mut odd = Vec::new();
    for one in opcodes() {
        assert!(!one.llvm.is_empty() && one.llvm.iter().all(|opcode| !opcode.is_empty()), "{}", one.name);
        if one.name.contains('.') {
            continue;
        }
        let listed = SPELLED.iter().find(|(name, _)| *name == one.name);
        let ok = match listed {
            Some((_, llvm)) => one.llvm.iter().all(|opcode| opcode.starts_with(llvm)),
            // LLVM spells a scalar conversion with a `U` (register) or `S`
            // (fixed-point) after its mnemonic, a compare with zero `ri`, and a
            // test of a bit by the width of the register: the producer's name
            // is the stem of each.
            None if FAMILIES.iter().any(|stem| one.name.starts_with(stem)) => {
                one.llvm.iter().all(|opcode| opcode.starts_with(&one.name[..4]))
            }
            None => one.llvm == [one.name.clone()],
        };
        if !ok {
            odd.push(format!("{} -> {:?}", one.name, one.llvm));
        }
    }
    assert!(odd.is_empty(), "{odd:#?}");
}

/// The rows of a name and its operand counts are one: two lines of the file
/// that make the same name and counts would hide the second.
#[test]
fn a_name_with_its_operand_counts_is_made_once() {
    let mut seen = std::collections::HashSet::new();
    for one in opcodes() {
        let key = (one.name.clone(), one.form.dests.len(), one.form.sources.len());
        assert!(seen.insert(key.clone()), "{key:?} is made twice");
    }
}
