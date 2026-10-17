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
use crate::context::{ConstantKind, mask};
use crate::module::{InstId, Instruction, Operand};
use crate::opcode::{BinaryOp, CastOp, IntPredicate, Opcode};

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

// nonzero.rs and power2.rs

/// The intrinsics ValueTracking names, matched by callee name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntrinsicId {
    Assume,
    Vscale,
    Ctpop,
    Ctlz,
    Cttz,
    Bswap,
    Bitreverse,
    Abs,
    Fshl,
    Fshr,
    SaddSat,
    UaddSat,
    SsubSat,
    UsubSat,
    SshlSat,
    UshlSat,
    Smin,
    Smax,
    Umin,
    Umax,
    UaddWithOverflow,
    SaddWithOverflow,
    UsubWithOverflow,
    SsubWithOverflow,
    UmulWithOverflow,
    SmulWithOverflow,
    LaunderInvariantGroup,
    Aarch64Irg,
    Aarch64Tagp,
    AmdgcnMakeBufferRsrc,
    Ptrmask,
    ThreadlocalAddress,
    ExperimentalGuard,
    ExperimentalGetVectorLength,
    RiscvVsetvlimax,
}

const INTRINSICS: [(&str, IntrinsicId); 35] = [
    ("assume", IntrinsicId::Assume),
    ("vscale", IntrinsicId::Vscale),
    ("ctpop", IntrinsicId::Ctpop),
    ("ctlz", IntrinsicId::Ctlz),
    ("cttz", IntrinsicId::Cttz),
    ("bswap", IntrinsicId::Bswap),
    ("bitreverse", IntrinsicId::Bitreverse),
    ("abs", IntrinsicId::Abs),
    ("fshl", IntrinsicId::Fshl),
    ("fshr", IntrinsicId::Fshr),
    ("sadd.sat", IntrinsicId::SaddSat),
    ("uadd.sat", IntrinsicId::UaddSat),
    ("ssub.sat", IntrinsicId::SsubSat),
    ("usub.sat", IntrinsicId::UsubSat),
    ("sshl.sat", IntrinsicId::SshlSat),
    ("ushl.sat", IntrinsicId::UshlSat),
    ("smin", IntrinsicId::Smin),
    ("smax", IntrinsicId::Smax),
    ("umin", IntrinsicId::Umin),
    ("umax", IntrinsicId::Umax),
    ("uadd.with.overflow", IntrinsicId::UaddWithOverflow),
    ("sadd.with.overflow", IntrinsicId::SaddWithOverflow),
    ("usub.with.overflow", IntrinsicId::UsubWithOverflow),
    ("ssub.with.overflow", IntrinsicId::SsubWithOverflow),
    ("umul.with.overflow", IntrinsicId::UmulWithOverflow),
    ("smul.with.overflow", IntrinsicId::SmulWithOverflow),
    ("launder.invariant.group", IntrinsicId::LaunderInvariantGroup),
    ("aarch64.irg", IntrinsicId::Aarch64Irg),
    ("aarch64.tagp", IntrinsicId::Aarch64Tagp),
    ("amdgcn.make.buffer.rsrc", IntrinsicId::AmdgcnMakeBufferRsrc),
    ("ptrmask", IntrinsicId::Ptrmask),
    ("threadlocal.address", IntrinsicId::ThreadlocalAddress),
    ("experimental.guard", IntrinsicId::ExperimentalGuard),
    ("experimental.get.vector.length", IntrinsicId::ExperimentalGetVectorLength),
    ("riscv.vsetvlimax", IntrinsicId::RiscvVsetvlimax),
];

/// The intrinsic `name` (`llvm.umax.i32`) is, without its type suffix.
pub fn intrinsic_id(name: &str) -> Option<IntrinsicId> {
    let rest = name.strip_prefix("llvm.")?;
    INTRINSICS
        .iter()
        .find(|(base, _)| rest.strip_prefix(base).is_some_and(|tail| tail.is_empty() || tail.starts_with('.')))
        .map(|(_, id)| *id)
}

/// `dyn_cast<IntrinsicInst>(I)->getIntrinsicID()` for a call (not an invoke).
pub fn call_intrinsic(
    q: &Query,
    inst: InstId,
) -> Option<IntrinsicId> {
    matches!(q.function.instruction(inst).opcode, Opcode::Call(_)).then_some(())?;
    intrinsic_id(q.callee_name(inst)?)
}

/// `m_Intrinsic<ID>(args...)`: `v` is a call to `id`: its instruction and its
/// arguments.
pub fn intrinsic<'a>(
    q: &Query<'a>,
    v: Operand,
    id: IntrinsicId,
) -> Option<(InstId, &'a Instruction)> {
    let (inst, instruction) = q.instruction_of(v)?;
    (call_intrinsic(q, inst) == Some(id)).then_some((inst, instruction))
}

/// `m_APInt`: the integer constant `v` is, and its width.
pub fn apint(
    q: &Query,
    v: Operand,
) -> Option<(u128, u32)> {
    q.int_constant(v)
}

/// `m_Zero`: the null pointer or the integer zero.
pub fn zero(
    q: &Query,
    v: Operand,
) -> bool {
    let Operand::Constant(id) = v else { return false };
    match q.context.get(id).kind {
        ConstantKind::Zero | ConstantKind::Null | ConstantKind::Int(0) => true,
        _ => false,
    }
}

/// `m_ZeroInt`.
pub fn zero_int(
    q: &Query,
    v: Operand,
) -> bool {
    matches!(apint(q, v), Some((0, _)))
}

/// `m_One`.
pub fn one(
    q: &Query,
    v: Operand,
) -> bool {
    matches!(apint(q, v), Some((1, _)))
}

/// `m_AllOnes`.
pub fn all_ones(
    q: &Query,
    v: Operand,
) -> bool {
    matches!(apint(q, v), Some((bits, width)) if bits == mask(width))
}

/// `m_Power2`.
pub fn power2(
    q: &Query,
    v: Operand,
) -> bool {
    matches!(apint(q, v), Some((bits, _)) if bits.is_power_of_two())
}

/// `m_Power2OrZero`.
pub fn power2_or_zero(
    q: &Query,
    v: Operand,
) -> bool {
    matches!(
        apint(q, v),
        Some((bits, _)) if bits == 0 || bits.is_power_of_two()
    )
}

/// `m_SignMask`.
pub fn sign_mask(
    q: &Query,
    v: Operand,
) -> bool {
    matches!(
        apint(q, v),
        Some((bits, width)) if bits == 1_u128 << (width - 1)
    )
}

/// `m_LowBitMask(C)`: the constant, when it is a non-zero run of low ones.
pub fn low_bit_mask(
    q: &Query,
    v: Operand,
) -> Option<(u128, u32)> {
    apint(q, v).filter(|(bits, _)| *bits != 0 && bits & bits.wrapping_add(1) == 0)
}

/// `m_ZExt`, `m_SExt`, `m_Trunc`, ... : `v` is `op x`: its instruction and `x`.
pub fn cast<'a>(
    q: &Query<'a>,
    v: Operand,
    op: CastOp,
) -> Option<(InstId, &'a Instruction, Operand)> {
    let (inst, instruction) = q.instruction_of(v)?;
    (instruction.opcode == Opcode::Cast(op)).then(|| (inst, instruction, instruction.operands[0]))
}

/// `m_ICmp(Pred, L, R)`: `v` is an integer comparison: its instruction,
/// predicate and operands.
pub fn icmp<'a>(
    q: &Query<'a>,
    v: Operand,
) -> Option<(InstId, &'a Instruction, IntPredicate, Operand, Operand)> {
    let (inst, instruction) = q.instruction_of(v)?;
    let Opcode::ICmp(pred) = instruction.opcode else { return None };
    Some((inst, instruction, pred, instruction.operands[0], instruction.operands[1]))
}

/// `m_c_ICmp(Pred, m_Specific(x), m_Value(Y))`: the predicate with `x` on the
/// left, and `Y`.
pub fn c_icmp_specific(
    q: &Query,
    v: Operand,
    x: Operand,
) -> Option<(IntPredicate, Operand)> {
    let (_, _, pred, lhs, rhs) = icmp(q, v)?;
    if lhs == x {
        Some((pred, rhs))
    } else if rhs == x {
        Some((pred.swapped(), lhs))
    } else {
        None
    }
}

/// `matchSimpleRecurrence(PN, BO, Start, Step)`: the binary operator, the
/// start value and the step.
pub fn match_simple_recurrence<'a>(
    q: &Query<'a>,
    phi: InstId,
) -> Option<(InstId, &'a Instruction, Operand, Operand)> {
    let instruction = q.function.instruction(phi);
    let result = Operand::Value(instruction.result?);
    if instruction.operands.len() != 4 {
        return None;
    }
    for i in 0..2 {
        let incoming = instruction.operands[2 * i];
        let Some((inst, operation)) = q.instruction_of(incoming) else { continue };
        if !matches!(operation.opcode, Opcode::Binary(_)) {
            continue;
        }
        let (lhs, rhs) = (operation.operands[0], operation.operands[1]);
        if lhs != result && rhs != result {
            continue;
        }
        let start = instruction.operands[2 * (1 - i)];
        return Some((inst, operation, start, if lhs == result { rhs } else { lhs }));
    }
    None
}
