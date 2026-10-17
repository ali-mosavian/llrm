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

// guarantee.rs

/// The LLVM intrinsics ValueTracking names that MIR does not enumerate in
/// `Intrinsic`, as `llvm.<name>` without the type suffix.
pub const INTRINSIC_NAMES: &[&str] = &[
    "sadd.with.overflow",
    "ssub.with.overflow",
    "smul.with.overflow",
    "uadd.with.overflow",
    "usub.with.overflow",
    "umul.with.overflow",
    "ctpop",
    "ctlz",
    "cttz",
    "abs",
    "smax",
    "smin",
    "umax",
    "umin",
    "scmp",
    "ucmp",
    "is.fpclass",
    "ptrmask",
    "bitreverse",
    "bswap",
    "sadd.sat",
    "ssub.sat",
    "sshl.sat",
    "uadd.sat",
    "usub.sat",
    "ushl.sat",
    "smul.fix",
    "smul.fix.sat",
    "umul.fix",
    "umul.fix.sat",
    "pow",
    "powi",
    "sin",
    "sinh",
    "cos",
    "cosh",
    "sincos",
    "sincospi",
    "tan",
    "tanh",
    "asin",
    "acos",
    "atan",
    "atan2",
    "canonicalize",
    "sqrt",
    "exp",
    "exp2",
    "exp10",
    "log",
    "log2",
    "log10",
    "modf",
    "floor",
    "ceil",
    "trunc",
    "rint",
    "nearbyint",
    "round",
    "roundeven",
    "lrint",
    "llrint",
    "fshl",
    "fshr",
    "frexp",
    "get.active.lane.mask",
    "matrix.column.major.load",
    "matrix.column.major.store",
];

/// The entry of [`INTRINSIC_NAMES`] a call to an `llvm.` function reaches: the
/// longest one the callee's name is, before its type suffix.
pub fn intrinsic_name(
    q: &Query,
    inst: InstId,
) -> Option<&'static str> {
    let name = q.callee_name(inst)?.strip_prefix("llvm.")?;
    INTRINSIC_NAMES
        .iter()
        .filter(|one| name.strip_prefix(**one).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
        .max_by_key(|one| one.len())
        .copied()
}

/// `m_ExtractValue(m_Value())`: `v` is an `extractvalue`: its instruction and
/// the aggregate it reads.
pub fn extract_value<'a>(
    q: &Query<'a>,
    v: Operand,
) -> Option<(InstId, &'a Instruction, Operand)> {
    let (inst, instruction) = q.instruction_of(v)?;
    matches!(instruction.opcode, Opcode::ExtractValue(_)).then(|| (inst, instruction, instruction.operands[0]))
}

/// `m_WithOverflowInst`: `v` is a call to a `*.with.overflow` intrinsic.
pub fn with_overflow_inst<'a>(
    q: &Query<'a>,
    v: Operand,
) -> Option<(InstId, &'a Instruction)> {
    let (inst, instruction) = q.instruction_of(v)?;
    matches!(
        q.intrinsic(inst),
        Some(crate::intrinsics::Intrinsic::WithOverflow { .. })
    ).then_some((inst, instruction))
}
