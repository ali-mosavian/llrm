//! LLVM's `PatternMatch.h` on MIR: the `m_*` matchers ValueTracking uses.
//!
//! # How it works
//! Each matcher takes an operand and answers what the operand is: the operands
//! of the binary operation it is, the constant it is. One function per `m_*`,
//! named for it. Add the matcher a translation needs here, at the end, under a
//! comment naming the file that needs it; two files needing one matcher share
//! it.
//!
//! # Example
//! `binary(q, v, BinaryOp::And)` on `%y = and i32 %a, 7` is `Some((%a, 7))`.

use super::query::Query;
use crate::module::{InstId, Instruction, Operand};
use crate::opcode::{BinaryOp, Opcode};

/// `m_BinOp`-family: `v` is `op a, b`: its instruction and both operands.
pub fn binary<'a>(
    q: &Query<'a>,
    v: Operand,
    op: BinaryOp,
) -> Option<(InstId, &'a Instruction, Operand, Operand)> {
    let (inst, instruction) = q.instruction_of(v)?;
    (instruction.opcode == Opcode::Binary(op))
        .then(|| (inst, instruction, instruction.operands[0], instruction.operands[1]))
}

// ----------------------------------------------------------------------------
// The intrinsics ValueTracking reads by name, which `Intrinsic` does not list.

/// `Intrinsic::ID` of the intrinsics ValueTracking asks about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntrinsicId {
    Abs,
    Smin,
    Smax,
    Umin,
    Umax,
    Ctpop,
    Ctlz,
    Cttz,
    Bswap,
    Bitreverse,
    Fshl,
    Fshr,
    SaddSat,
    UaddSat,
    SsubSat,
    UsubSat,
    SaddWithOverflow,
    UaddWithOverflow,
    SsubWithOverflow,
    UsubWithOverflow,
    SmulWithOverflow,
    UmulWithOverflow,
}

const INTRINSIC_NAMES: &[(&str, IntrinsicId)] = &[
    ("llvm.abs", IntrinsicId::Abs),
    ("llvm.smin", IntrinsicId::Smin),
    ("llvm.smax", IntrinsicId::Smax),
    ("llvm.umin", IntrinsicId::Umin),
    ("llvm.umax", IntrinsicId::Umax),
    ("llvm.ctpop", IntrinsicId::Ctpop),
    ("llvm.ctlz", IntrinsicId::Ctlz),
    ("llvm.cttz", IntrinsicId::Cttz),
    ("llvm.bswap", IntrinsicId::Bswap),
    ("llvm.bitreverse", IntrinsicId::Bitreverse),
    ("llvm.fshl", IntrinsicId::Fshl),
    ("llvm.fshr", IntrinsicId::Fshr),
    ("llvm.sadd.sat", IntrinsicId::SaddSat),
    ("llvm.uadd.sat", IntrinsicId::UaddSat),
    ("llvm.ssub.sat", IntrinsicId::SsubSat),
    ("llvm.usub.sat", IntrinsicId::UsubSat),
    ("llvm.sadd.with.overflow", IntrinsicId::SaddWithOverflow),
    ("llvm.uadd.with.overflow", IntrinsicId::UaddWithOverflow),
    ("llvm.ssub.with.overflow", IntrinsicId::SsubWithOverflow),
    ("llvm.usub.with.overflow", IntrinsicId::UsubWithOverflow),
    ("llvm.smul.with.overflow", IntrinsicId::SmulWithOverflow),
    ("llvm.umul.with.overflow", IntrinsicId::UmulWithOverflow),
];

/// `IntrinsicInst::getIntrinsicID` for the intrinsics above: `inst` is a call
/// to one, its type suffix aside.
pub fn intrinsic_id(
    q: &Query,
    inst: InstId,
) -> Option<IntrinsicId> {
    let name = q.callee_name(inst)?;
    INTRINSIC_NAMES
        .iter()
        .find(|(base, _)| name.strip_prefix(base).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
        .map(|(_, id)| *id)
}
