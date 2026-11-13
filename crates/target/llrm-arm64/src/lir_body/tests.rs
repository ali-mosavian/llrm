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
use crate::mc::inst::{
    Addr, Arrangement, Gpr, ImmShape, NarrowLane, VectorFma, VectorFpOp, VectorFpUnary, VectorLongOp, VectorOp,
    VectorReduce, VectorShift,
};
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
pub(super) fn instructions() -> Vec<Inst> {
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
        for (subtract, set_flags) in [(false, false), (false, true), (true, false), (true, true)] {
            out.push(Inst::AddCarry { op: AddSub { subtract, set_flags }, width, rd, rn, rm });
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
        if size != FpSize::H {
            for signed in [false, true] {
                out.push(Inst::FpLaneToFp { signed, size, rd: v(0), rn: v(1) });
            }
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
    for op in [VectorOp::Add, VectorOp::Mul, VectorOp::And, VectorOp::Eor, VectorOp::Ushl] {
        for lanes in [Arrangement::B16, Arrangement::H8, Arrangement::S4, Arrangement::D2] {
            if (op.is_logical() && lanes != Arrangement::B16) || (op == VectorOp::Mul && lanes == Arrangement::D2) {
                continue;
            }
            out.push(Inst::VectorBinary { op, lanes, rd: v(0), rn: v(1), rm: v(2) });
        }
    }
    for op in [VectorShift::Shl, VectorShift::Ushr, VectorShift::Sshr] {
        out.push(Inst::VectorShift { op, lanes: Arrangement::H8, rd: v(0), rn: v(1), amount: 3 });
    }
    for lane in [NarrowLane::B, NarrowLane::H, NarrowLane::S] {
        for high in [false, true] {
            for op in [VectorLongOp::Ushll, VectorLongOp::Sshll] {
                out.push(Inst::VectorLongShift { op, lane, high, rd: v(0), rn: v(1), amount: 0 });
            }
            for op in [VectorLongOp::Uaddw, VectorLongOp::Saddw, VectorLongOp::Umull, VectorLongOp::Smull] {
                out.push(Inst::VectorLong { op, lane, high, rd: v(0), rn: v(1), rm: v(2) });
            }
        }
    }
    out.push(Inst::VectorNot { rd: v(0), rn: v(1) });
    for (op, lanes) in [(VectorFma::Add, Arrangement::S4), (VectorFma::Sub, Arrangement::D2)] {
        out.push(Inst::VectorFma { op, lanes, rd: v(0), rn: v(1), rm: v(2) });
        out.push(Inst::VectorFmaLane { op, lanes, rd: v(0), rn: v(1), rm: v(2), index: 1 });
    }
    out.push(Inst::VectorFmulLane { lanes: Arrangement::S4, rd: v(0), rn: v(1), rm: v(2), index: 1 });
    for lanes in [Arrangement::B16, Arrangement::H8, Arrangement::S4, Arrangement::D2] {
        out.push(Inst::VectorInsGpr { lanes, rd: v(0), rn: Gpr(3), index: 1 });
        out.push(Inst::VectorInsLane { lanes, rd: v(0), rn: v(1), index: 1, src: 0 });
        out.push(Inst::VectorUmov { lanes, rd: Gpr(3), rn: v(1), index: 1 });
        out.push(Inst::VectorStoreLane { lanes, rt: v(1), index: 1, base: Gpr(3) });
    }
    for (op, lanes) in [
        (VectorReduce::Addv, Arrangement::B16),
        (VectorReduce::Smaxv, Arrangement::H8),
        (VectorReduce::Uminv, Arrangement::S4),
        (VectorReduce::Addp, Arrangement::D2),
    ] {
        out.push(Inst::VectorReduce { op, lanes, rd: v(0), rn: v(1) });
    }
    out.push(Inst::VectorDupLane { lanes: Arrangement::S4, rd: v(0), rn: v(1), index: 0 });
    out.push(Inst::VectorDupLane { lanes: Arrangement::D2, rd: v(0), rn: v(1), index: 1 });
    for lanes in [Arrangement::S4, Arrangement::D2] {
        for op in [VectorFpOp::Add, VectorFpOp::Sub, VectorFpOp::Mul, VectorFpOp::Div] {
            out.push(Inst::VectorFpBinary { op, lanes, rd: v(0), rn: v(1), rm: v(2) });
        }
        for op in [
            VectorFpUnary::Neg,
            VectorFpUnary::Abs,
            VectorFpUnary::Scvtf,
            VectorFpUnary::Ucvtf,
            VectorFpUnary::Fcvtzs,
            VectorFpUnary::Fcvtzu,
        ] {
            out.push(Inst::VectorFpUnary { op, lanes, rd: v(0), rn: v(1) });
        }
    }
    for fill in [
        Fill::Zero,
        Fill::Byte(0xa5),
        Fill::Dup(Arrangement::B16, rn),
        Fill::Dup(Arrangement::H8, rn),
        Fill::Dup(Arrangement::D2, rn),
        Fill::Imm { invert: false, shape: ImmShape::H8(8), imm8: 7 },
        Fill::Imm { invert: true, shape: ImmShape::S4(16), imm8: 0xa5 },
        Fill::Imm { invert: false, shape: ImmShape::S4Msl(8), imm8: 3 },
        Fill::Mask(0b1010_0101),
    ] {
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
        Inst::AddCarry { .. } => "AddCarry",
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
        Inst::FpLaneToFp { .. } => "FpLaneToFp",
        Inst::FpInt { .. } => "FpInt",
        Inst::FpFixed { .. } => "FpFixed",
        Inst::VectorFill { .. } => "VectorFill",
        Inst::VectorBinary { .. } => "VectorBinary",
        Inst::VectorShift { .. } => "VectorShift",
        Inst::VectorNot { .. } => "VectorNot",
        Inst::VectorLongShift { .. } => "VectorLongShift",
        Inst::VectorLong { .. } => "VectorLong",
        Inst::VectorFma { .. } => "VectorFma",
        Inst::VectorInsGpr { .. } => "VectorInsGpr",
        Inst::VectorInsLane { .. } => "VectorInsLane",
        Inst::VectorUmov { .. } => "VectorUmov",
        Inst::VectorStoreLane { .. } => "VectorStoreLane",
        Inst::VectorFmaLane { .. } => "VectorFmaLane",
        Inst::VectorFmulLane { .. } => "VectorFmulLane",
        Inst::VectorReduce { .. } => "VectorReduce",
        Inst::VectorDupLane { .. } => "VectorDupLane",
        Inst::VectorFpBinary { .. } => "VectorFpBinary",
        Inst::VectorFpUnary { .. } => "VectorFpUnary",
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

const ALL: [&str; 63] = [
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
    "AddCarry",
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
    "FpLaneToFp",
    "FpInt",
    "FpFixed",
    "VectorFill",
    "VectorBinary",
    "VectorShift",
    "VectorNot",
    "VectorLongShift",
    "VectorLong",
    "VectorFma",
    "VectorInsGpr",
    "VectorInsLane",
    "VectorUmov",
    "VectorStoreLane",
    "VectorFmaLane",
    "VectorFmulLane",
    "VectorReduce",
    "VectorDupLane",
    "VectorFpBinary",
    "VectorFpUnary",
    "Svc",
    "Atomic",
    "Dmb",
    "Brk",
    "Nop",
    "Prefetch",
];

pub(super) fn function(ops: Vec<Op>) -> MFunc {
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
/// functions of the corpus: "the general registers' byte lanes left: 15 right:
/// 72".
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
    let float_registers =
        SIZES.map(|size| Op::I(Inst::FpBinary { op: FpBinary::Add, size, rd: Vreg(0), rn: Vreg(1), rm: Vreg(2) }));
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
    let machine = |list: crate::mc::visit::RegList| {
        list.into_iter().filter(|r| !r.is_virtual() && *r != Reg::Gpr(Gpr::R31)).count()
    };
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

/// A load whose instruction carries no memory operand is refused on the way
/// back rather than given an invented one.
#[test]
fn a_load_rebuilt_without_its_memory_operand_is_refused() {
    let load = Inst::Load {
        mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: g(0) },
        addr: Addr::Offset { base: g(1), imm: 0 },
        mmo: MemOperand::default(),
    };
    let function = function(vec![Op::I(load)]);
    let mut lowered = to_lir(&function).expect("maps");
    lowered
        .body
        .blocks[0]
        .insns
        .edit(
            |insns| for one in insns.iter_mut() {
                let mut bare = (**one).clone();
                bare.memory = None;
                *one = Arc::new(bare);
            },
        );
    let Err(Unmapped(why)) = from_lir(&lowered, &function) else { panic!("rebuilt a load with no memory operand") };
    assert!(why.starts_with("LDR"), "{why}");
}

/// The shared scheduler's graph ordered a load after every earlier store (the
/// memory lane is all arm64 stated), so a load from one frame slot waited for
/// a store to another. The instruction's own memory operand now tells them
/// apart; the same slot stays ordered.
#[test]
fn a_load_from_another_slot_is_not_ordered_after_a_store() {
    let at =
        |slot: u32| MemOperand { object: crate::mc::inst::MemObject::Slot(slot), offset: Some(0), volatile: false };
    let store = |slot: u32| {
        Op::I(Inst::Store {
            mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: g(0) },
            addr: Addr::Offset { base: g(1), imm: 0 },
            mmo: at(slot),
        })
    };
    let load = |slot: u32, into: u32| {
        Op::I(Inst::Load {
            mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: g(into) },
            addr: Addr::Offset { base: g(1), imm: 0 },
            mmo: at(slot),
        })
    };
    let lowered = to_lir(&function(vec![store(1), load(2, 2), load(1, 3)])).expect("maps");
    let _file = enter(Regs(&crate::REGISTER_INFO));
    let graph = llrm_codegen::backend::machinesched::graph(
        &lowered.body.blocks[0].insns,
        Regs(&crate::REGISTER_INFO),
        64,
        &llrm_codegen::backend::schedmodel::GenericModel,
        None,
    );
    assert!(graph.preds[1].is_empty(), "a load of slot 2 waits for a store to slot 1: {:?}", graph.preds);
    assert_eq!(graph.preds[2], [0], "a load of slot 1 follows the store to it");
}

/// The allocator's own path (`lir_view` -> the spike converter -> the
/// scheduler's body) carried only the memory lane, so the scheduler's graph
/// ordered a load after a store to another slot; `lir_body::to_lir`, which the
/// check path uses, was not the producer the default compile runs.
#[test]
fn the_default_producer_tells_the_scheduler_what_an_access_reaches() {
    let at =
        |slot: u32| MemOperand { object: crate::mc::inst::MemObject::Slot(slot), offset: Some(0), volatile: false };
    let store = |slot: u32| {
        Op::I(Inst::Store {
            mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: g(0) },
            addr: Addr::Offset { base: g(1), imm: 0 },
            mmo: at(slot),
        })
    };
    let load = |slot: u32, into: u32| {
        Op::I(Inst::Load {
            mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: g(into) },
            addr: Addr::Offset { base: g(1), imm: 0 },
            mmo: at(slot),
        })
    };
    let function = function(vec![store(1), load(2, 2), load(1, 3)]);
    let registers = Regs(&crate::REGISTER_INFO);
    let view = crate::lir_view::view(&function);
    let converted = llrm_spike_mf::tolir::to_lir::<crate::lir_view::Arm64>(&view, registers).expect("converts");
    let _file = enter(registers);
    let body = converted.with_effects(&view, registers);
    let graph = llrm_codegen::backend::machinesched::graph(
        &body.blocks[0].insns,
        registers,
        body.bits,
        &llrm_codegen::backend::schedmodel::GenericModel,
        None,
    );
    assert!(graph.preds[1].is_empty(), "a load of slot 2 waits for a store to slot 1: {:?}", graph.preds);
    assert_eq!(graph.preds[2], [0], "a load of slot 1 follows the store to it");
}

/// An arm64 register prints by its file's name. `Debug` was iced's, so `x0`
/// showed as `AL` and `x1` as `R12W` in every dump and assertion of arm64 code.
#[test]
fn a_register_prints_by_the_arm64_names() {
    let _file = enter(Regs(&crate::REGISTER_INFO));
    assert_eq!(format!("{:?} {:?} {:?}", crate::reg::X0, crate::reg::W3, crate::reg::V1), "x0 w3 v1");
}

/// A three-address `subs w, w, #imm` is not tied: the x86 rule (any `Binary`
/// writes its first source) made the O0 two-address pass put a `mov` before
/// every arm64 add, subtract and logic op. Only the forms whose row ties an
/// operand (`MOVK`'s `^0`) are tied.
#[test]
fn only_a_row_with_a_tied_operand_is_untied_by_the_two_address_pass() {
    use llrm_codegen::backend::twoaddr::{ties_in, untied_only};

    let (rd, rn) = (Gpr(Gpr::FIRST_VIRTUAL + 1), Gpr(Gpr::FIRST_VIRTUAL + 2));
    let width = WIDTHS[0];
    let op = AddSub { subtract: true, set_flags: true };
    let body = to_lir(&function(vec![
        Op::I(Inst::AddSubImm { op, width, rd, rn, imm12: 4, shift12: false }),
        Op::I(Inst::MoveWide { op: Wide::Keep, width, rd: rn, imm16: 0xbeef, lsl: 16 }),
    ]))
    .expect("the forms map")
    .body;
    let regs = Some(Regs(&crate::REGISTER_INFO));
    let tied: Vec<(Option<String>, bool)> = body
        .iter_insns()
        .filter_map(|one| one.what.as_ref())
        .map(|what| (what.name.as_deref().map(str::to_owned), ties_in(regs, what)))
        .collect();
    assert!(tied.contains(&(Some("SUBSWri".into()), false)), "{tied:?}");
    assert!(
        tied.iter().any(|(name, tied)| name.as_deref().is_some_and(|n| n.starts_with("MOVK")) && *tied),
        "{tied:?}"
    );
    let after = untied_only(&body);
    assert_eq!(after.iter_insns().count(), body.iter_insns().count(), "a copy was put before an untied form");
}

/// A base set by a copy and then updated by a post-indexed load is one register
/// defined twice. The body gives the update's result a value of its own (the
/// pin's `$wback`), so it is SSA: the verifier said "defined 2 times in a body
/// that should be in SSA" for 3 functions of the -O1 corpus (`main`, `fmt`) and
/// the shared SSA phases read two definitions of one value. The way back puts
/// the register back.
#[test]
fn a_writeback_base_is_a_value_of_its_own_in_the_body() {
    use llrm_target::Target as _;
    let (base, loaded, stepped) = (g(0), g(1), g(2));
    let function = function(vec![
        Op::Copy { dst: Reg::Gpr(stepped), src: Reg::Gpr(base) },
        Op::I(Inst::Load {
            mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: loaded },
            addr: Addr::Post { base: stepped, imm: 8 },
            mmo: Default::default(),
        }),
        Op::Copy { dst: Reg::Gpr(Gpr(0)), src: Reg::Gpr(stepped) },
    ]);
    let lowered = to_lir(&function).expect("maps");
    let roles = &crate::description::Arm64.layout().spaces.roles;
    let twice: Vec<_> = llrm_codegen::backend::verify::verify_for(&lowered.body, true, roles)
        .into_iter()
        .filter(|one| one.contains("defined 2 times"))
        .collect();
    assert_eq!(twice, Vec::<String>::new());
    assert_eq!(round_trip(&function).expect("round-trips").blocks[0].ops.len(), 3);
}

/// A value an `fadd` defines is confined to the float registers. The class
/// rule knew `xmm` only, so an arm64 `d` value was free to take a general
/// register (3 of the corpus's 43 functions got one for a 16-byte value).
#[test]
fn a_float_operand_is_confined_to_the_float_registers() {
    use llrm_codegen::backend::classes::RegisterClasses;
    use llrm_codegen::backend::regclass::{Confinement, Sets, confinement_of};

    let size = FpSize::D;
    let rd = Vreg(Gpr::FIRST_VIRTUAL + 3);
    let lowered = to_lir(&function(vec![Op::I(Inst::FpBinary {
        op: FpBinary::Add,
        size,
        rd,
        rn: Vreg(Gpr::FIRST_VIRTUAL + 4),
        rm: Vreg(Gpr::FIRST_VIRTUAL + 5),
    })]))
    .expect("an fadd maps");
    let classes = RegisterClasses::of(&crate::description::Arm64);
    let sets = Sets::of(&classes);
    let defined = lowered.body.iter_insns().find_map(|one| one.defines.first().copied()).expect("a definition");
    let found = confinement_of(lowered.body.regs(), lowered.body.iter_insns().map(|one| &**one), defined, &sets);
    match found {
        Confinement::Only(only) => {
            assert!(
                !only.is_empty()
                    && only.iter().all(|one| lowered.body.regs().in_class(*one, llrm_lir::registers::class::FPR)),
                "{only:?}"
            )
        }
        _ => panic!("an fadd's result is not confined to the float registers"),
    }
}

/// A shift of two values the loop never writes is made before the loop. The
/// pin: MachineLICM moves arm64 forms (it changed 0 of 43 corpus functions
/// only because the mid-end had already hoisted them).
#[test]
fn an_invariant_shift_in_a_loop_is_made_before_it() {
    use llrm_codegen::backend::classes::RegisterClasses;
    use llrm_codegen::backend::machinelicm::hoisted;

    let r = |n| Gpr(Gpr::FIRST_VIRTUAL + n);
    let (amount, value, shifted, counter) = (r(1), r(2), r(3), r(4));
    let wide = |rd| Inst::MoveWide { op: Wide::Zero, width: WIDTHS[1], rd, imm16: 7, lsl: 0 };
    let mut function = MFunc::new("t");
    function.next_register = Gpr::FIRST_VIRTUAL + 16;
    let block =
        |label: &str, ops: Vec<Op>| MBlock { label: label.into(), ops: ops.into_iter().map(|op| (op, None)).collect() };
    function.blocks = vec![
        block("pre", vec![Op::I(wide(amount)), Op::I(wide(value)), Op::I(wide(counter))]),
        block(
            "loop",
            vec![
                Op::I(Inst::Divide { op: Div::Lslv, width: WIDTHS[1], rd: shifted, rn: value, rm: amount }),
                Op::I(Inst::AddSubImm {
                    op: AddSub { subtract: true, set_flags: false },
                    width: WIDTHS[1],
                    rd: counter,
                    rn: counter,
                    imm12: 1,
                    shift12: false,
                }),
                Op::I(Inst::CompareBranch { nonzero: true, width: Width::X, rt: counter, target: label("loop") }),
            ],
        ),
        block("done", vec![Op::Return { uses: vec![], tail: None }]),
    ];
    let body = to_lir(&function).expect("the forms map").body;
    let after = hoisted(&body, &RegisterClasses::of(&crate::description::Arm64));
    let in_block = |body: &LirBody, at: usize, name: &str| {
        body.blocks[at].insns.iter().any(|one| one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some(name)))
    };
    assert!(in_block(&body, 1, "LSLVXr"), "the premise: the shift starts in the loop");
    assert!(in_block(&after, 0, "LSLVXr") && !in_block(&after, 1, "LSLVXr"), "the shift stayed in the loop");
}

/// `v4` is a float register, not a segment register: the one-address-space
/// check listed x86's segment ids, and the 128-bit `v4` is id 71, x86's `ES`
/// (`v5` is `CS`...), so a function copying a wide value (sum, mix, fmt)
/// carried 8-10 inherited complaints.
#[test]
fn a_float_register_is_not_a_segment_register() {
    use llrm_target::Target as _;
    let (a, b) = (Vreg(4), Vreg(5));
    let mut function = function(vec![Op::Copy { dst: Reg::Fpr(a), src: Reg::Fpr(b) }]);
    function.wide = vec![Reg::Fpr(a), Reg::Fpr(b)];
    let lowered = to_lir(&function).expect("a wide copy maps");
    let complaints = llrm_codegen::backend::verify::verify_for(
        &lowered.body,
        false,
        &crate::description::Arm64.layout().spaces.roles,
    );
    let segment: Vec<_> = complaints.iter().filter(|one| one.contains("segment")).collect();
    assert!(segment.is_empty(), "{segment:?}");
}

/// The shared MachineSink splits an edge only when its probability is at most
/// 40% (`SPLIT_EDGE_PROBABILITY_THRESHOLD`). The body carried no odds, so every
/// edge read as even and the entry -> join edge of a `icmp sgt a, 0` branch
/// (37.5% in the pin) was never split: a constant stayed in the entry.
#[test]
fn the_body_carries_the_edge_odds_by_block_position() {
    use llrm_mir::blockfrequency::BranchProbability;
    let mut one = function(vec![Op::I(Inst::Nop)]);
    one.alloc_odds = Some([((0, 1), (BranchProbability::new(3, 8), 0))].into_iter().collect());
    let lowered = to_lir(&one).expect("a nop maps");
    assert_eq!(lowered.body.odds.probability(0, 1), Some(0.375));
    assert_eq!(lowered.body.odds.probability(1, 0), None);
}

/// A constant made inside a loop is made once in its preheader: the pin hoists
/// every trivially rematerializable instruction (`IsProfitableToHoist`, "should
/// always be hoisted providing the register allocator can just pull them down
/// again"). The predicate asked for the x86 name `mov`, so arm64's `MOVImm`
/// stayed in the loop (nbody_fixed's `movz #512` in the innermost loop, +29995
/// executed instructions at -O2 once the sink stage ran).
#[test]
fn a_constant_made_in_a_loop_is_made_in_its_preheader() {
    use llrm_codegen::backend::classes::RegisterClasses;
    use llrm_codegen::backend::machinelicm::hoisted;

    let r = |n| Gpr(Gpr::FIRST_VIRTUAL + n);
    let (value, divisor, quotient, counter) = (r(2), r(1), r(3), r(4));
    let wide = |dst, value| Op::Imm { dst, bits: 64, value };
    let mut function = MFunc::new("t");
    function.next_register = Gpr::FIRST_VIRTUAL + 16;
    let block =
        |label: &str, ops: Vec<Op>| MBlock { label: label.into(), ops: ops.into_iter().map(|op| (op, None)).collect() };
    function.blocks = vec![
        block("pre", vec![wide(value, 7), wide(counter, 9)]),
        block(
            "loop",
            vec![
                wide(divisor, 512),
                Op::I(Inst::Divide { op: Div::Udiv, width: WIDTHS[1], rd: quotient, rn: value, rm: divisor }),
                Op::I(Inst::AddSubImm {
                    op: AddSub { subtract: true, set_flags: false },
                    width: WIDTHS[1],
                    rd: counter,
                    rn: counter,
                    imm12: 1,
                    shift12: false,
                }),
                Op::I(Inst::CompareBranch { nonzero: true, width: Width::X, rt: counter, target: label("loop") }),
            ],
        ),
        block("done", vec![Op::Return { uses: vec![], tail: None }]),
    ];
    let body = to_lir(&function).expect("the forms map").body;
    let after = hoisted(&body, &RegisterClasses::of(&crate::description::Arm64));
    let has = |body: &LirBody, at: usize, name: &str| {
        body.blocks[at].insns.iter().any(|one| one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some(name)))
    };
    assert!(has(&body, 1, "MOVImm"), "the premise: the constant starts in the loop");
    assert!(has(&after, 0, "MOVImm") && !has(&after, 1, "MOVImm"), "the constant stayed in the loop");
}

/// A constant a copy in the loop reads, and no phi takes, is made once in the
/// preheader: `HasLoopPHIUse` looks past a copy only to a phi
/// (MachineLICM.cpp#L1166). Any copy of the value kept it in the loop, so
/// nbody_fixed's `movz #512` (selected as `movz; mov`) stayed in the innermost
/// loop of the sink stage (+29997 executed instructions at -O2).
#[test]
fn a_constant_a_loop_copies_is_made_in_the_preheader() {
    use llrm_codegen::backend::classes::RegisterClasses;
    use llrm_codegen::backend::machinelicm::hoisted;

    let r = |n| Gpr(Gpr::FIRST_VIRTUAL + n);
    let (value, constant, divisor, quotient, counter) = (r(2), r(5), r(1), r(3), r(4));
    let wide = |dst, value| Op::Imm { dst, bits: 64, value };
    let mut function = MFunc::new("t");
    function.next_register = Gpr::FIRST_VIRTUAL + 16;
    let block =
        |label: &str, ops: Vec<Op>| MBlock { label: label.into(), ops: ops.into_iter().map(|op| (op, None)).collect() };
    function.blocks = vec![
        block("pre", vec![wide(value, 7), wide(counter, 9)]),
        block(
            "loop",
            vec![
                wide(constant, 512),
                Op::Copy { dst: Reg::Gpr(divisor), src: Reg::Gpr(constant) },
                Op::I(Inst::Divide { op: Div::Udiv, width: WIDTHS[1], rd: quotient, rn: value, rm: divisor }),
                Op::I(Inst::AddSubImm {
                    op: AddSub { subtract: true, set_flags: false },
                    width: WIDTHS[1],
                    rd: counter,
                    rn: counter,
                    imm12: 1,
                    shift12: false,
                }),
                Op::I(Inst::CompareBranch { nonzero: true, width: Width::X, rt: counter, target: label("loop") }),
            ],
        ),
        block("done", vec![Op::Return { uses: vec![], tail: None }]),
    ];
    let body = to_lir(&function).expect("the forms map").body;
    let after = hoisted(&body, &RegisterClasses::of(&crate::description::Arm64));
    let has = |body: &LirBody, at: usize, name: &str| {
        body.blocks[at].insns.iter().any(|one| one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some(name)))
    };
    assert!(has(&body, 1, "MOVImm"), "the premise: the constant starts in the loop");
    assert!(has(&after, 0, "MOVImm") && !has(&after, 1, "MOVImm"), "the constant stayed in the loop");
}
