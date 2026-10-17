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

// ---- context.rs ----

use crate::opcode::{CastOp, Flags, IntPredicate};

/// `CmpPredicate`: a predicate and whether the operands have one sign.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CmpPredicate {
    pub pred: IntPredicate,
    pub same_sign: bool,
}

impl CmpPredicate {
    pub fn new(pred: IntPredicate) -> Self {
        Self { pred, same_sign: false }
    }

    /// `CmpPredicate::getInverse`.
    pub fn inverse(self) -> Self {
        Self { pred: self.pred.inverse(), same_sign: self.same_sign }
    }

    /// `CmpPredicate::getSwapped`.
    pub fn swapped(self) -> Self {
        Self { pred: self.pred.swapped(), same_sign: self.same_sign }
    }

    /// `ICmpInst::getFlippedSignednessPredicate`.
    pub fn flipped_signedness(pred: IntPredicate) -> IntPredicate {
        match pred {
            IntPredicate::Eq | IntPredicate::Ne => pred,
            IntPredicate::Ugt => IntPredicate::Sgt,
            IntPredicate::Uge => IntPredicate::Sge,
            IntPredicate::Ult => IntPredicate::Slt,
            IntPredicate::Ule => IntPredicate::Sle,
            IntPredicate::Sgt => IntPredicate::Ugt,
            IntPredicate::Sge => IntPredicate::Uge,
            IntPredicate::Slt => IntPredicate::Ult,
            IntPredicate::Sle => IntPredicate::Ule,
        }
    }

    /// `CmpPredicate::getMatching`.
    pub fn get_matching(
        a: Self,
        b: Self,
    ) -> Option<Self> {
        if a.pred == b.pred {
            return Some(if a.same_sign == b.same_sign { a } else { Self::new(a.pred) });
        }
        if a.same_sign && a.pred == Self::flipped_signedness(b.pred) {
            return Some(Self::new(b.pred));
        }
        if b.same_sign && b.pred == Self::flipped_signedness(a.pred) {
            return Some(Self::new(a.pred));
        }
        None
    }

    /// `CmpPredicate::getPreferredSignedPredicate`.
    pub fn preferred_signed_predicate(self) -> IntPredicate {
        if !self.same_sign {
            return self.pred;
        }
        match self.pred {
            IntPredicate::Ugt => IntPredicate::Sgt,
            IntPredicate::Uge => IntPredicate::Sge,
            IntPredicate::Ult => IntPredicate::Slt,
            IntPredicate::Ule => IntPredicate::Sle,
            other => other,
        }
    }
}

/// An operand of a comparison `m_ICmpLike` matched: a value, or the integer
/// zero it makes up for `trunc nuw x to i1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Value(Operand),
    Int(u128, u32),
}

impl Side {
    /// An integer constant is always an `Int`, as LLVM's uniqued `ConstantInt`
    /// is one object.
    pub fn of(
        q: &Query,
        v: Operand,
    ) -> Self {
        match q.int_constant(v) {
            Some((bits, width)) => Self::Int(bits, width),
            None => Self::Value(v),
        }
    }

    /// `m_APInt`: the integer constant and its width.
    pub fn apint(
        self,
        q: &Query,
    ) -> Option<(u128, u32)> {
        match self {
            Self::Value(v) => q.int_constant(v),
            Self::Int(bits, width) => Some((bits, width)),
        }
    }
}

/// `m_Zero`: the null value of its type (zero, `null`).
pub fn zero(
    q: &Query,
    v: Operand,
) -> bool {
    use crate::context::ConstantKind;
    let Operand::Constant(id) = v else { return false };
    match q.context().get(id).kind {
        ConstantKind::Zero | ConstantKind::Null | ConstantKind::Int(0) => true,
        ConstantKind::Float(bits) => bits == 0,
        _ => false,
    }
}

/// `m_ImmConstant`: a constant that is no constant expression.
pub fn imm_constant(
    q: &Query,
    v: Operand,
) -> bool {
    use crate::context::ConstantKind;
    matches!(
        v,
        Operand::Constant(id) if !matches!(q.context().get(id).kind, ConstantKind::Expr(_))
    )
}

/// `m_ConstantInt(uint64_t &)`: an integer constant that fits 64 bits.
pub fn constant_int_u64(
    q: &Query,
    v: Operand,
) -> Option<u64> {
    q.int_constant(v).and_then(|(bits, _)| u64::try_from(bits).ok())
}

/// `m_Power2(const APInt *&)`.
pub fn power2_constant(
    q: &Query,
    v: Operand,
) -> Option<u128> {
    q.int_constant(v).map(|(bits, _)| bits).filter(|bits| bits.is_power_of_two())
}

/// `m_Trunc`, `m_PtrToInt`, ...: `v` is `op x`, with the instruction and `x`.
pub fn cast<'a>(
    q: &Query<'a>,
    v: Operand,
    op: CastOp,
) -> Option<(InstId, &'a Instruction, Operand)> {
    let (inst, instruction) = q.instruction_of(v)?;
    (instruction.opcode == Opcode::Cast(op)).then(|| (inst, instruction, instruction.operands[0]))
}

/// `m_c_<Op>(P, m_Value(Other))`: `v` is `op a, b` where `a` or `b` satisfies
/// `first`, with the other. `a` is tried first.
pub fn commutative_with<'a>(
    q: &Query<'a>,
    v: Operand,
    op: BinaryOp,
    first: impl Fn(Operand) -> bool,
) -> Option<(InstId, &'a Instruction, Operand)> {
    let (inst, instruction, a, b) = binary(q, v, op)?;
    if first(a) {
        Some((inst, instruction, b))
    } else if first(b) {
        Some((inst, instruction, a))
    } else {
        None
    }
}

/// `m_Not`: `xor x, -1` in either order, with `x`.
pub fn not(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    let all_ones = |x: Operand| q.int_constant(x).is_some_and(|(bits, width)| bits == crate::context::mask(width));
    commutative_with(q, v, BinaryOp::Xor, all_ones).map(|(_, _, other)| other)
}

/// `m_Select`: the condition and both arms.
pub fn select<'a>(
    q: &Query<'a>,
    v: Operand,
) -> Option<(InstId, &'a Instruction, Operand, Operand, Operand)> {
    let (inst, instruction) = q.instruction_of(v)?;
    (instruction.opcode == Opcode::Select)
        .then(|| (inst, instruction, instruction.operands[0], instruction.operands[1], instruction.operands[2]))
}

/// `m_ICmp(Pred, ..)`: the instruction, its predicate (with `samesign`) and
/// both operands.
pub fn icmp<'a>(
    q: &Query<'a>,
    v: Operand,
) -> Option<(InstId, &'a Instruction, CmpPredicate, Operand, Operand)> {
    let (inst, instruction) = q.instruction_of(v)?;
    let Opcode::ICmp(pred) = instruction.opcode else { return None };
    let same_sign = instruction.flags.contains(Flags::SAMESIGN);
    Some((inst, instruction, CmpPredicate { pred, same_sign }, instruction.operands[0], instruction.operands[1]))
}

/// `m_ICmpLike`: an `icmp`, or `trunc nuw x to i1` as `icmp ne x, 0`.
pub fn icmp_like(
    q: &Query,
    v: Operand,
) -> Option<(CmpPredicate, Side, Side)> {
    if let Some((_, _, pred, a, b)) = icmp(q, v) {
        return Some((pred, Side::of(q, a), Side::of(q, b)));
    }
    let (_, instruction, x) = cast(q, v, CastOp::Trunc)?;
    if q.scalar_bits(v) != 1 || !instruction.flags.contains(Flags::NUW) {
        return None;
    }
    Some((CmpPredicate::new(IntPredicate::Ne), Side::of(q, x), Side::Int(0, q.scalar_bits(x))))
}

/// `m_LogicalAnd`: `a & b` or `a ? b : false`, on `i1`.
pub fn logical_and(
    q: &Query,
    v: Operand,
) -> Option<(Operand, Operand)> {
    let (_, instruction) = q.instruction_of(v)?;
    if q.scalar_bits(v) != 1 {
        return None;
    }
    if instruction.opcode == Opcode::Binary(BinaryOp::And) {
        return Some((instruction.operands[0], instruction.operands[1]));
    }
    let (_, _, cond, t, f) = select(q, v)?;
    (q.type_of(cond) == q.type_of(v) && zero(q, f)).then_some((cond, t))
}

/// `m_LogicalOr`: `a | b` or `a ? true : b`, on `i1`.
pub fn logical_or(
    q: &Query,
    v: Operand,
) -> Option<(Operand, Operand)> {
    let (_, instruction) = q.instruction_of(v)?;
    if q.scalar_bits(v) != 1 {
        return None;
    }
    if instruction.opcode == Opcode::Binary(BinaryOp::Or) {
        return Some((instruction.operands[0], instruction.operands[1]));
    }
    let (_, _, cond, t, f) = select(q, v)?;
    let one = q.int_constant(t).is_some_and(|(bits, width)| bits == crate::context::mask(width));
    (q.type_of(cond) == q.type_of(v) && one).then_some((cond, f))
}

/// `m_LogicalOp`: a logical and, else a logical or.
pub fn logical_op(
    q: &Query,
    v: Operand,
) -> Option<(Operand, Operand)> {
    logical_and(q, v).or_else(|| logical_or(q, v))
}

/// `m_AddLike`-family: `add` with the `wrap` flags (none for `m_AddLike`,
/// `NSW` for `m_NSWAddLike`, `NUW` for `m_NUWAddLike`), or `or disjoint`.
pub fn add_like(
    q: &Query,
    v: Operand,
    wrap: Flags,
) -> Option<(Operand, Operand)> {
    if let Some((_, instruction, a, b)) = binary(q, v, BinaryOp::Add)
        && instruction.flags.contains(wrap)
    {
        return Some((a, b));
    }
    disjoint_or(q, v)
}

/// `m_DisjointOr`.
pub fn disjoint_or(
    q: &Query,
    v: Operand,
) -> Option<(Operand, Operand)> {
    let (_, instruction, a, b) = binary(q, v, BinaryOp::Or)?;
    instruction.flags.contains(Flags::DISJOINT).then_some((a, b))
}

/// `m_Intrinsic<..>(..)`: `v` is a call of the intrinsic `base` (`llvm.smax`,
/// its type suffix aside), with its arguments.
pub fn intrinsic_call<'a>(
    q: &Query<'a>,
    v: Operand,
    base: &str,
) -> Option<(InstId, &'a [Operand])> {
    let (inst, instruction) = q.instruction_of(v)?;
    let name = q.callee_name(inst)?;
    let rest = name.strip_prefix(base)?;
    (rest.is_empty() || rest.starts_with('.')).then(|| (inst, &instruction.operands[..instruction.operands.len() - 1]))
}

/// `m_c_Intrinsic<..>(P, m_Value())`: a two-argument call of `base` where
/// either argument satisfies `first`, with the other.
pub fn commutative_intrinsic_with(
    q: &Query,
    v: Operand,
    base: &str,
    first: impl Fn(Operand) -> bool,
) -> Option<Operand> {
    let (_, args) = intrinsic_call(q, v, base)?;
    let [a, b] = *args else { return None };
    if first(a) {
        Some(b)
    } else if first(b) {
        Some(a)
    } else {
        None
    }
}

/// `m_FNeg`: `fneg x`, or `fsub -0.0, x` (any zero with `nsz`).
pub fn fneg(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    use crate::context::ConstantKind;
    use crate::types::{FloatKind, Type};
    let (_, instruction) = q.instruction_of(v)?;
    match instruction.opcode {
        Opcode::FNeg => Some(instruction.operands[0]),
        Opcode::Binary(BinaryOp::FSub) => {
            let Operand::Constant(id) = instruction.operands[0] else { return None };
            let constant = q.context().get(id);
            let ConstantKind::Float(bits) = constant.kind else { return None };
            let Type::Float(kind) = q.context().types.get(constant.ty) else { return None };
            let sign = if *kind == FloatKind::Float { 1_u64 << 31 } else { 1_u64 << 63 };
            let matched = if instruction.flags.contains(Flags::NSZ) { bits & !sign == 0 } else { bits == sign };
            matched.then(|| instruction.operands[1])
        }
        _ => None,
    }
}
