//! Every form the producer maps, built by hand: the corpus does not reach all
//! of them (no pair before allocation, no `movk`, few atomics), and a form it
//! does not reach is a form whose round trip nothing checks.

use llrm_codegen::backend::lanes::{Lanes, enter};
use llrm_codegen::model::lir::SrcLoc;
use llrm_lir::registers::Regs;
use llrm_mir::MetadataId;

use super::*;
use crate::mc::inst::{
    AddSub, Atomic, Barrier, Bitfield, CcOperand, Cond, Div, Extend, Fill, FpBinary, FpConvert, FpFused, FpSize,
    FpUnary, Logic, Memory, Mul3, Rounding, Select, Unary, Wide,
};
use crate::mc::inst::{Addr, Gpr};
use crate::mfunc::Callee;

fn g(n: u32) -> Gpr {
    Gpr(Gpr::FIRST_VIRTUAL + n)
}

fn v(n: u32) -> Vreg {
    Vreg(Gpr::FIRST_VIRTUAL + n)
}

fn label(text: &str) -> Target {
    Target::Label(text.into())
}

fn symbol(text: &str) -> Target {
    Target::Symbol { name: text.into(), addend: 8 }
}

const WIDTHS: [Width; 2] = [Width::W, Width::X];
const SIZES: [FpSize; 3] = [FpSize::S, FpSize::D, FpSize::H];

/// One instance of each member of each closed set an instruction carries.
fn instructions() -> Vec<Inst> {
    let mut out = Vec::new();
    let (rd, rn, rm, ra) = (g(0), Gpr(3), g(1), Gpr(31));
    for width in WIDTHS {
        for (subtract, set_flags) in [(false, false), (true, false), (false, true), (true, true)] {
            let op = AddSub { subtract, set_flags };
            out.push(Inst::AddSubImm { op, width, rd, rn, imm12: 4095, shift12: true });
            out.push(Inst::AddSubShifted { op, width, rd, rn, rm, shift: Shift::Asr, amount: 7 });
            out.push(Inst::AddSubExtended { op, width, rd, rn, rm, extend: Extend::Sxtw, amount: 2 });
        }
        for op in [Logic::And, Logic::Orr, Logic::Eor, Logic::Ands] {
            out.push(Inst::LogicImm { op, width, rd, rn, imm: 0x00ff_00ff_00ff_00ff });
            for invert in [false, true] {
                out.push(Inst::LogicShifted { op, invert, width, rd, rn, rm, shift: Shift::Ror, amount: 3 });
            }
        }
        for op in [Wide::Not, Wide::Zero, Wide::Keep] {
            out.push(Inst::MoveWide { op, width, rd, imm16: 0xbeef, lsl: 16 });
        }
        for op in [Bitfield::Sbfm, Bitfield::Bfm, Bitfield::Ubfm] {
            out.push(Inst::BitfieldMove { op, width, rd, rn, immr: 3, imms: 9 });
        }
        out.push(Inst::Extract { width, rd, rn, rm, lsb: 5 });
        for op in [Div::Udiv, Div::Sdiv, Div::Lslv, Div::Lsrv, Div::Asrv, Div::Rorv] {
            out.push(Inst::Divide { op, width, rd, rn, rm });
        }
        for op in [Unary::Rbit, Unary::Rev16, Unary::Rev, Unary::Clz, Unary::Cls] {
            out.push(Inst::OneSource { op, width, rd, rn });
        }
        for op in [Select::Csel, Select::Csinc, Select::Csinv, Select::Csneg] {
            out.push(Inst::CondSelect { op, width, rd, rn, rm, cond: Cond::Le });
        }
        for negate in [false, true] {
            out.push(Inst::CondCompare { negate, width, rn, rm_or_imm: CcOperand::Reg(rm), nzcv: 4, cond: Cond::Hi });
            out.push(Inst::CondCompare { negate, width, rn, rm_or_imm: CcOperand::Imm5(17), nzcv: 2, cond: Cond::Al });
        }
    }
    for op in [Mul3::Madd, Mul3::Msub, Mul3::Smaddl, Mul3::Smsubl, Mul3::Smulh, Mul3::Umaddl, Mul3::Umsubl, Mul3::Umulh]
    {
        out.push(Inst::ThreeSource { op, width: Width::X, rd, rn, rm, ra });
    }
    out.push(Inst::AddPageOffset { rd, rn, target: symbol("table") });
    out.push(Inst::AddPageOffset { rd, rn, target: label(".LCPI0_0") });
    for page in [false, true] {
        out.push(Inst::Adr { page, rd, target: symbol("global"), got: false });
    }
    out.push(Inst::Adr { page: true, rd, target: symbol("global"), got: true });
    out.push(Inst::Adr { page: false, rd, target: label(".LCPI0_0"), got: false });
    // Control.
    out.push(Inst::Ret { rn: Gpr::LR });
    out.push(Inst::Branch { link: false, target: label("b") });
    out.push(Inst::Branch { link: true, target: symbol("callee") });
    out.push(Inst::BranchCond { cond: Cond::Ne, target: label("b") });
    for nonzero in [false, true] {
        out.push(Inst::CompareBranch { nonzero, width: Width::W, rt: rd, target: label("b") });
        out.push(Inst::TestBranch { nonzero, rt: rd, bit: 40, target: label("b") });
    }
    out.push(Inst::BranchReg { link: false, rn });
    out.push(Inst::BranchReg { link: true, rn });
    out.push(Inst::Svc { imm16: 0x80 });
    out.push(Inst::Brk { imm16: 1 });
    out.push(Inst::Nop);
    out.push(Inst::Prefetch { op: 3, base: rn });
    for barrier in [Barrier::Ish, Barrier::IshLd, Barrier::IshSt] {
        out.push(Inst::Dmb { barrier });
    }
    // Memory: every access kind in every address form.
    let memories = [
        Memory::Int { width: Width::W, bytes: 1, sign: false, reg: rd },
        Memory::Int { width: Width::X, bytes: 2, sign: true, reg: rd },
        Memory::Int { width: Width::X, bytes: 4, sign: true, reg: rd },
        Memory::Int { width: Width::X, bytes: 8, sign: false, reg: rd },
        Memory::Fp { bytes: 4, reg: v(0) },
        Memory::Fp { bytes: 16, reg: v(0) },
    ];
    let addresses = [
        Addr::Offset { base: rn, imm: -8 },
        Addr::Pre { base: g(2), imm: 16 },
        Addr::Post { base: g(2), imm: -16 },
        Addr::Register { base: rn, index: rm, extend: Extend::Uxtw, shifted: true },
        Addr::Slot { slot: 2, offset: 24 },
        Addr::Slot { slot: crate::mfunc::INCOMING_ARGUMENTS, offset: 8 },
        Addr::PageOffset { base: rn, target: symbol("global"), got: false },
        Addr::PageOffset { base: rn, target: symbol("global"), got: true },
    ];
    let mmo = MemOperand { object: crate::mc::inst::MemObject::Slot(2), offset: Some(24), volatile: true };
    for mem in memories {
        for addr in &addresses {
            out.push(Inst::Load { mem, addr: addr.clone(), mmo: mmo.clone() });
            out.push(Inst::Store { mem, addr: addr.clone(), mmo: mmo.clone() });
        }
    }
    // A pair's two registers are one kind (`ldpsw`, `ldp x`, `ldp w`, `ldp d`,
    // `ldp q`).
    let pair = |kind: Memory, other: Memory| (kind, other);
    for (first, second) in [
        pair(
            Memory::Int { width: Width::X, bytes: 4, sign: true, reg: rd },
            Memory::Int { width: Width::X, bytes: 4, sign: true, reg: g(3) },
        ),
        pair(memories[3], Memory::Int { width: Width::X, bytes: 8, sign: false, reg: g(3) }),
        pair(
            Memory::Int { width: Width::W, bytes: 4, sign: false, reg: rd },
            Memory::Int { width: Width::W, bytes: 4, sign: false, reg: g(3) },
        ),
        pair(Memory::Fp { bytes: 8, reg: v(0) }, Memory::Fp { bytes: 8, reg: v(1) }),
        pair(memories[5], Memory::Fp { bytes: 16, reg: v(1) }),
    ] {
        for addr in &addresses[..3] {
            out.push(Inst::LoadPair { first, second, addr: addr.clone() });
            out.push(Inst::StorePair { first, second, addr: addr.clone() });
        }
    }
    for op in [
        Atomic::Ldar,
        Atomic::Ldapr,
        Atomic::Stlr,
        Atomic::Swp,
        Atomic::Ldadd,
        Atomic::Ldclr,
        Atomic::Ldeor,
        Atomic::Ldset,
        Atomic::Ldsmax,
        Atomic::Ldsmin,
        Atomic::Ldumax,
        Atomic::Ldumin,
        Atomic::Cas,
    ] {
        let plain = matches!(op, Atomic::Ldar | Atomic::Ldapr | Atomic::Stlr);
        let rs = if plain { Gpr::R31 } else { g(5) };
        out.push(Inst::Atomic { op, size: 3, acquire: true, release: !plain, rs, rt: g(6), rn: g(7) });
    }
    // Floating point.
    for size in SIZES {
        for op in [
            FpUnary::Mov,
            FpUnary::Abs,
            FpUnary::Neg,
            FpUnary::Sqrt,
            FpUnary::Rint(Rounding::Nearest),
            FpUnary::Rint(Rounding::Up),
            FpUnary::Rint(Rounding::Down),
            FpUnary::Rint(Rounding::Zero),
            FpUnary::Rint(Rounding::Away),
            FpUnary::Rint(Rounding::Exact),
            FpUnary::Rint(Rounding::Current),
        ] {
            out.push(Inst::FpUnary { op, size, rd: v(0), rn: v(1) });
        }
        for op in [
            FpBinary::Mul,
            FpBinary::Div,
            FpBinary::Add,
            FpBinary::Sub,
            FpBinary::Max,
            FpBinary::Min,
            FpBinary::MaxNm,
            FpBinary::MinNm,
            FpBinary::Nmul,
        ] {
            out.push(Inst::FpBinary { op, size, rd: v(0), rn: v(1), rm: v(2) });
        }
        for op in [FpFused::Madd, FpFused::Msub, FpFused::Nmadd, FpFused::Nmsub] {
            out.push(Inst::FpFused { op, size, rd: v(0), rn: v(1), rm: v(2), ra: v(3) });
        }
        for signalling in [false, true] {
            out.push(Inst::FpCompare { signalling, size, rn: v(1), rm: Some(v(2)) });
            out.push(Inst::FpCompare { signalling, size, rn: v(1), rm: None });
        }
        out.push(Inst::FpSelect { size, rd: v(0), rn: v(1), rm: v(2), cond: Cond::Lt });
        out.push(Inst::FpMoveImm { size, rd: v(0), imm8: 0x70 });
        for from in SIZES {
            out.push(Inst::FpToFp { to: size, from, rd: v(0), rn: v(1) });
        }
        for width in WIDTHS {
            for op in [
                FpConvert::ToFp { signed: true },
                FpConvert::ToFp { signed: false },
                FpConvert::ToIntZero { signed: true },
                FpConvert::ToIntZero { signed: false },
                FpConvert::Bits { to_fp: true },
                FpConvert::Bits { to_fp: false },
            ] {
                out.push(Inst::FpInt { op, width, size, gpr: rd, fpr: v(0) });
                if !matches!(op, FpConvert::Bits { .. }) {
                    out.push(Inst::FpFixed { op, width, size, gpr: rd, fpr: v(0), fbits: 12 });
                }
            }
        }
    }
    out.push(Inst::VectorMov { rd: v(0), rn: v(1) });
    for fill in [Fill::Zero, Fill::Byte(0xa5), Fill::Gpr(rn)] {
        out.push(Inst::VectorFill { rd: v(0), fill });
    }
    out
}

/// The variant's name; the match has no wildcard, so a new variant of [`Inst`]
/// does not compile until it is listed here and in [`instructions`].
fn variant(inst: &Inst) -> &'static str {
    match inst {
        Inst::Mark(_) => "Mark",
        Inst::Label(_) => "Label",
        Inst::Cfi(_) => "Cfi",
        Inst::AddSubImm { .. } => "AddSubImm",
        Inst::AddSubShifted { .. } => "AddSubShifted",
        Inst::AddSubExtended { .. } => "AddSubExtended",
        Inst::AddPageOffset { .. } => "AddPageOffset",
        Inst::LogicImm { .. } => "LogicImm",
        Inst::LogicShifted { .. } => "LogicShifted",
        Inst::MoveWide { .. } => "MoveWide",
        Inst::BitfieldMove { .. } => "BitfieldMove",
        Inst::Extract { .. } => "Extract",
        Inst::Divide { .. } => "Divide",
        Inst::OneSource { .. } => "OneSource",
        Inst::ThreeSource { .. } => "ThreeSource",
        Inst::CondSelect { .. } => "CondSelect",
        Inst::CondCompare { .. } => "CondCompare",
        Inst::Branch { .. } => "Branch",
        Inst::BranchCond { .. } => "BranchCond",
        Inst::CompareBranch { .. } => "CompareBranch",
        Inst::TestBranch { .. } => "TestBranch",
        Inst::BranchReg { .. } => "BranchReg",
        Inst::Ret { .. } => "Ret",
        Inst::Load { .. } => "Load",
        Inst::Store { .. } => "Store",
        Inst::LoadPair { .. } => "LoadPair",
        Inst::StorePair { .. } => "StorePair",
        Inst::Adr { .. } => "Adr",
        Inst::FpUnary { .. } => "FpUnary",
        Inst::VectorMov { .. } => "VectorMov",
        Inst::FpBinary { .. } => "FpBinary",
        Inst::FpFused { .. } => "FpFused",
        Inst::FpCompare { .. } => "FpCompare",
        Inst::FpSelect { .. } => "FpSelect",
        Inst::FpMoveImm { .. } => "FpMoveImm",
        Inst::FpToFp { .. } => "FpToFp",
        Inst::FpInt { .. } => "FpInt",
        Inst::FpFixed { .. } => "FpFixed",
        Inst::VectorFill { .. } => "VectorFill",
        Inst::Svc { .. } => "Svc",
        Inst::Atomic { .. } => "Atomic",
        Inst::Dmb { .. } => "Dmb",
        Inst::Brk { .. } => "Brk",
        Inst::Nop => "Nop",
        Inst::Prefetch { .. } => "Prefetch",
    }
}

/// The variants after frame lowering, which the body is never made from.
const AFTER_FRAME: [&str; 3] = ["Mark", "Label", "Cfi"];

const ALL: [&str; 45] = [
    "Mark",
    "Label",
    "Cfi",
    "AddSubImm",
    "AddSubShifted",
    "AddSubExtended",
    "AddPageOffset",
    "LogicImm",
    "LogicShifted",
    "MoveWide",
    "BitfieldMove",
    "Extract",
    "Divide",
    "OneSource",
    "ThreeSource",
    "CondSelect",
    "CondCompare",
    "Branch",
    "BranchCond",
    "CompareBranch",
    "TestBranch",
    "BranchReg",
    "Ret",
    "Load",
    "Store",
    "LoadPair",
    "StorePair",
    "Adr",
    "FpUnary",
    "VectorMov",
    "FpBinary",
    "FpFused",
    "FpCompare",
    "FpSelect",
    "FpMoveImm",
    "FpToFp",
    "FpInt",
    "FpFixed",
    "VectorFill",
    "Svc",
    "Atomic",
    "Dmb",
    "Brk",
    "Nop",
    "Prefetch",
];

fn function(ops: Vec<Op>) -> MFunc {
    let mut function = MFunc::new("t");
    function.slots = vec![crate::mfunc::Slot { size: 16, align: 8 }; 4];
    function.next_register = Gpr::FIRST_VIRTUAL + 16;
    let places: Vec<_> =
        ops.iter().enumerate().map(|(at, op)| (op.clone(), Some(SrcLoc::line(at as u32 + 1)))).collect();
    function.blocks = vec![
        MBlock { label: "a".into(), ops: places.into_iter().collect() },
        MBlock { label: "b".into(), ops: vec![Op::Return { uses: vec![], tail: None }].into() },
    ];
    function
}

/// Every instruction form, in every member of each closed set it carries,
/// round-trips with its registers, its place and the body's defines and uses.
/// Before this test the corpus alone covered 226 of the forms and nothing
/// named the rest.
#[test]
fn every_instruction_form_round_trips() {
    let all = instructions();
    let seen: std::collections::BTreeSet<_> = all.iter().map(variant).collect();
    let missing: Vec<_> = ALL.iter().filter(|name| !seen.contains(*name) && !AFTER_FRAME.contains(name)).collect();
    assert!(missing.is_empty(), "{missing:?} have no instance in the test");
    let function = function(all.into_iter().map(Op::I).collect());
    round_trip(&function).expect("every instruction round-trips");
}

/// The body names arm64's register file. It had none, so the first shared
/// phase run on it (`PhiElimination`) panicked: every stage of every function
/// of the corpus read "the body has no register file".
#[test]
fn the_body_carries_the_arm64_register_file() {
    let lowered = to_lir(&function(vec![Op::I(Inst::Nop)])).expect("a nop maps");
    assert!(std::ptr::eq(lowered.body.regs().0, &crate::REGISTER_INFO));
}

/// The verifier accepts what the producer makes, for every form and width.
/// It named a register at a width that register is not (`x3` at 32 where the
/// file has `w3`) in the first operand of every function of the 24 programs, so
/// every shared phase's output was malformed by inheritance.
#[test]
fn the_producer_names_registers_at_their_own_width() {
    use llrm_target::Target as _;
    let all = instructions();
    let lowered = to_lir(&function(all.into_iter().map(Op::I).collect())).expect("every form maps");
    let complaints = llrm_codegen::backend::verify::verify_for(
        &lowered.body,
        false,
        &crate::description::Arm64.layout().spaces.roles,
    );
    let widths: Vec<_> = complaints.iter().filter(|one| one.contains("not the width")).collect();
    assert!(widths.is_empty(), "{} of {}: {:?}", widths.len(), complaints.len(), widths.first());
}

/// No shared phase panics on arm64's body. `CopyPropagation` (and `Peephole`
/// through it) asserted x86's 72 byte-lane units, so it panicked on all 43
/// functions of the corpus: "the general registers' byte lanes left: 15 right: 72".
#[test]
fn no_shared_phase_panics_on_the_producers_body() {
    let lowered = to_lir(&function(vec![Op::I(Inst::Nop)])).expect("a nop maps");
    let (_, stages) = stages::run(lowered.body).expect("the phases build");
    let panicked: Vec<_> = stages.iter().filter(|one| matches!(one.outcome, stages::Outcome::Panicked(_))).collect();
    assert!(panicked.is_empty(), "{panicked:?}");
}

/// Every register the producer names, float ones included, is one the file
/// describes. The verifier knew integer, segment and XMM registers only, so
/// the `d`/`s`/`h` views of arm64's float code were "not a register this
/// target has" in 6 of the 43 functions of the corpus.
#[test]
fn the_producer_names_only_registers_the_file_describes() {
    use llrm_target::Target as _;
    let float_registers = SIZES.map(|size| {
        Op::I(Inst::FpBinary { op: FpBinary::Add, size, rd: Vreg(0), rn: Vreg(1), rm: Vreg(2) })
    });
    let lowered = to_lir(&function(float_registers.into())).expect("a float add maps");
    let complaints = llrm_codegen::backend::verify::verify_for(
        &lowered.body,
        false,
        &crate::description::Arm64.layout().spaces.roles,
    );
    let unknown: Vec<_> = complaints.iter().filter(|one| one.contains("not a register this target has")).collect();
    assert!(unknown.is_empty(), "{} of {}: {:?}", unknown.len(), complaints.len(), unknown.first());
}

/// The operations that are not instructions.
#[test]
fn every_pseudo_operation_round_trips() {
    let ops = vec![
        Op::Copy { dst: Reg::Gpr(g(0)), src: Reg::Gpr(Gpr(1)) },
        Op::Copy { dst: Reg::Fpr(v(0)), src: Reg::Fpr(Vreg(8)) },
        Op::Imm { dst: g(1), bits: 32, value: 0xdead_beef },
        Op::Imm { dst: g(1), bits: 64, value: u64::MAX },
        Op::Call {
            callee: Callee::Symbol("f".into()),
            uses: vec![Reg::Gpr(Gpr(0)), Reg::Fpr(Vreg(0))],
            defs: vec![Reg::Gpr(Gpr(0))],
            tail: false,
        },
        Op::Call { callee: Callee::Register(g(3)), uses: vec![], defs: vec![Reg::Fpr(Vreg(0))], tail: true },
        Op::FrameAddress { dst: g(2), slot: 1, offset: 8 },
        Op::FrameAddress { dst: g(2), slot: crate::mfunc::INCOMING_ARGUMENTS, offset: 16 },
        Op::DbgValue { variable: MetadataId(7), reg: Some(Reg::Gpr(g(2))) },
        Op::DbgValue { variable: MetadataId(8), reg: None },
        Op::JumpTable {
            dst: g(4),
            scratch: g(5),
            table: g(6),
            index: g(7),
            id: 3,
            targets: vec!["b".into(), "a".into(), "b".into()],
        },
        Op::Return { uses: vec![Reg::Gpr(Gpr(0)), Reg::Fpr(Vreg(0))], tail: Some(Callee::Symbol("g".into())) },
        Op::Return { uses: vec![], tail: Some(Callee::Register(Gpr(16))) },
    ];
    round_trip(&function(ops)).expect("every pseudo operation round-trips");
}

/// A form after frame lowering is refused by name, never mapped by guess.
#[test]
fn forms_after_the_frame_are_refused_by_name() {
    for (inst, name) in
        [(Inst::Label("l".into()), "Label"), (Inst::Mark(crate::mc::inst::FrameMark::PrologueEnd), "Mark")]
    {
        let why = round_trip(&function(vec![Op::I(inst)])).expect_err("refused");
        assert!(why.contains(&format!("unmapped form {name}")), "{why}");
    }
}

/// An instruction states what it reads and writes: the status flags where it
/// sets or reads them, memory where it loads or stores, the machine registers
/// it names and no others. The body stated none before (`declared` was
/// `None`), so a shared pass saw every arm64 instruction as of unknown effect.
#[test]
fn every_instruction_states_its_effect() {
    let mut ops: Vec<Op> = instructions().into_iter().map(Op::I).collect();
    ops.push(Op::Call {
        callee: Callee::Symbol("f".into()),
        uses: vec![Reg::Gpr(Gpr(0))],
        defs: vec![Reg::Gpr(Gpr(1))],
        tail: false,
    });
    let function = function(ops);
    let lowered = to_lir(&function).expect("maps");
    let _file = enter(Regs(&crate::REGISTER_INFO));
    let (flags, memory) = (Lanes::flags(1), Lanes::flags(Lanes::MEMORY));
    let machine = |list: Vec<Reg>| list.into_iter().filter(|r| !r.is_virtual() && *r != Reg::Gpr(Gpr::R31)).count();
    for (op, insn) in function.blocks[0].ops.iter().zip(lowered.body.blocks[0].insns.iter()) {
        let (reads, writes) = insn.declared.unwrap_or_else(|| panic!("{op:?} states no effect"));
        let facts = crate::facts::of(op);
        let says = |lanes: &Lanes, lane: &Lanes| lane.is_subset(lanes);
        assert_eq!(says(&reads, &flags), facts.reads_flags, "{op:?} reads the flags");
        assert_eq!(says(&writes, &flags), facts.sets_flags, "{op:?} sets the flags");
        assert_eq!(says(&reads, &memory), facts.may_load || facts.may_store || facts.call, "{op:?} reads memory");
        assert_eq!(says(&writes, &memory), facts.may_store || facts.call, "{op:?} writes memory");
        assert!(insn.volatile || !(facts.side_effects || facts.call), "{op:?} is volatile");
        let (defs, uses) = op.defs_and_uses();
        let strip = |lanes: Lanes| lanes.minus(&flags).minus(&memory);
        // One lane per machine register: a register named twice is one lane.
        assert!(strip(writes).iter().count() <= machine(defs), "{op:?} writes lanes it does not name");
        assert!(strip(reads).iter().count() <= machine(uses), "{op:?} reads lanes it does not name");
    }
    // The call's argument and result registers are lanes.
    let call = lowered.body.blocks[0].insns.last().unwrap();
    let (reads, writes) = call.declared.unwrap();
    assert!(!reads.minus(&memory).is_empty() && !writes.minus(&memory).is_empty());
}

/// A 128-bit float register copied is 128 bits in the body; it was 64, so a
/// shared spiller would have saved half of an `f128`.
#[test]
fn a_wide_copy_is_128_bits() {
    let mut function = function(vec![Op::Copy { dst: Reg::Fpr(v(0)), src: Reg::Fpr(v(1)) }]);
    function.wide = vec![Reg::Fpr(v(0)), Reg::Fpr(v(1))];
    let lowered = to_lir(&function).expect("maps");
    let what = lowered.body.blocks[0].insns[0].what.as_ref().unwrap();
    assert_eq!(what.dests.iter().chain(&what.sources).map(bits_of).collect::<Vec<_>>(), vec![128, 128]);
}

/// A call touches any memory and is not moved across; a trap is not moved
/// across either (LLVM: `isCall`, `BRK` `hasSideEffects`). Before this, facts
/// gave both none, so a shared stage that read the lanes could move a store
/// past a call.
#[test]
fn a_call_and_a_trap_are_barriers() {
    let call = Op::Call { callee: Callee::Symbol("f".into()), uses: vec![], defs: vec![], tail: false };
    let trap = Op::I(Inst::Brk { imm16: 1 });
    let lowered = to_lir(&function(vec![call, trap])).expect("maps");
    let _file = enter(Regs(&crate::REGISTER_INFO));
    let memory = Lanes::flags(Lanes::MEMORY);
    let insns = &lowered.body.blocks[0].insns;
    let (reads, writes) = insns[0].declared.unwrap();
    assert!(memory.is_subset(&reads) && memory.is_subset(&writes), "a call reads and writes memory");
    assert!(insns[0].volatile && insns[1].volatile);
}

/// The side tables stand in for the instruction's own memory operand: a body
/// rebuilt without them refuses a load rather than inventing one (DEVIATE 5).
#[test]
fn a_load_rebuilt_without_its_memory_operand_is_refused() {
    let load = Inst::Load {
        mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: g(0) },
        addr: Addr::Offset { base: g(1), imm: 0 },
        mmo: MemOperand::default(),
    };
    let function = function(vec![Op::I(load)]);
    let mut lowered = to_lir(&function).expect("maps");
    lowered.memory_operands.clear();
    let Err(Unmapped(why)) = from_lir(&lowered, &function) else { panic!("rebuilt a load with no memory operand") };
    assert!(why.starts_with("LDR"), "{why}");
}

/// An arm64 register prints by its file's name. `Debug` was iced's, so `x0`
/// showed as `AL` and `x1` as `R12W` in every dump and assertion of arm64 code.
#[test]
fn a_register_prints_by_the_arm64_names() {
    let _file = enter(Regs(&crate::REGISTER_INFO));
    assert_eq!(format!("{:?} {:?} {:?}", crate::reg::X0, crate::reg::W3, crate::reg::V1), "x0 w3 v1");
}
