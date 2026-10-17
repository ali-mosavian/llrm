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

// ---- intrinsics by name

/// `Intrinsic::ID` of the intrinsics ValueTracking tells apart. MIR has none
/// of them but the min/max, with-overflow and assume ones, so a call is
/// matched by its callee's name with the `.iN` type suffix dropped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntrinsicId {
    /// Not a call to an `llvm.*` function.
    NotIntrinsic,
    /// An intrinsic not listed here.
    Other,
    Assume,
    Abs,
    Ctlz,
    Cttz,
    Ctpop,
    Frexp,
    UAddSat,
    SAddSat,
    USubSat,
    SSubSat,
    UMin,
    UMax,
    SMin,
    SMax,
    MinNum,
    MaxNum,
    Minimum,
    Maximum,
    MinimumNum,
    MaximumNum,
    Vscale,
}

const INTRINSIC_NAMES: [(&str, IntrinsicId); 21] = [
    ("llvm.assume", IntrinsicId::Assume),
    ("llvm.abs", IntrinsicId::Abs),
    ("llvm.ctlz", IntrinsicId::Ctlz),
    ("llvm.cttz", IntrinsicId::Cttz),
    ("llvm.ctpop", IntrinsicId::Ctpop),
    ("llvm.frexp", IntrinsicId::Frexp),
    ("llvm.uadd.sat", IntrinsicId::UAddSat),
    ("llvm.sadd.sat", IntrinsicId::SAddSat),
    ("llvm.usub.sat", IntrinsicId::USubSat),
    ("llvm.ssub.sat", IntrinsicId::SSubSat),
    ("llvm.umin", IntrinsicId::UMin),
    ("llvm.umax", IntrinsicId::UMax),
    ("llvm.smin", IntrinsicId::SMin),
    ("llvm.smax", IntrinsicId::SMax),
    ("llvm.minnum", IntrinsicId::MinNum),
    ("llvm.maxnum", IntrinsicId::MaxNum),
    ("llvm.minimum", IntrinsicId::Minimum),
    ("llvm.maximum", IntrinsicId::Maximum),
    ("llvm.minimumnum", IntrinsicId::MinimumNum),
    ("llvm.maximumnum", IntrinsicId::MaximumNum),
    ("llvm.vscale", IntrinsicId::Vscale),
];

/// `Function::getIntrinsicID` of the function named `name`.
pub fn intrinsic_id(name: &str) -> IntrinsicId {
    INTRINSIC_NAMES
        .iter()
        .find(|(base, _)| name.strip_prefix(base).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
        .map_or_else(
            || if name.starts_with("llvm.") { IntrinsicId::Other } else { IntrinsicId::NotIntrinsic },
            |(_, id)| *id,
        )
}
