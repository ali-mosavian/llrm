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

// guarantee.rs

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

// ---- the intrinsics ValueTracking reads by name ----

macro_rules! intrinsics {
    ($($variant:ident => $name:literal,)*) => {
        /// `Intrinsic::ID` of the intrinsics ValueTracking asks about; MIR
        /// enumerates few of them, so they are found by callee name.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum IntrinsicId {
            $($variant,)*
        }

        const INTRINSIC_NAMES: &[(&str, IntrinsicId)] = &[$(($name, IntrinsicId::$variant),)*];
    };
}

intrinsics! {
    SaddWithOverflow => "sadd.with.overflow",
    SsubWithOverflow => "ssub.with.overflow",
    SmulWithOverflow => "smul.with.overflow",
    UaddWithOverflow => "uadd.with.overflow",
    UsubWithOverflow => "usub.with.overflow",
    UmulWithOverflow => "umul.with.overflow",
    Ctpop => "ctpop",
    Ctlz => "ctlz",
    Cttz => "cttz",
    Abs => "abs",
    Smax => "smax",
    Smin => "smin",
    Umax => "umax",
    Umin => "umin",
    Scmp => "scmp",
    Ucmp => "ucmp",
    IsFpclass => "is.fpclass",
    Ptrmask => "ptrmask",
    Bitreverse => "bitreverse",
    Bswap => "bswap",
    SaddSat => "sadd.sat",
    SsubSat => "ssub.sat",
    SshlSat => "sshl.sat",
    UaddSat => "uadd.sat",
    UsubSat => "usub.sat",
    UshlSat => "ushl.sat",
    SmulFix => "smul.fix",
    SmulFixSat => "smul.fix.sat",
    UmulFix => "umul.fix",
    UmulFixSat => "umul.fix.sat",
    Pow => "pow",
    Powi => "powi",
    Sin => "sin",
    Sinh => "sinh",
    Cos => "cos",
    Cosh => "cosh",
    Sincos => "sincos",
    Sincospi => "sincospi",
    Tan => "tan",
    Tanh => "tanh",
    Asin => "asin",
    Acos => "acos",
    Atan => "atan",
    Atan2 => "atan2",
    Canonicalize => "canonicalize",
    Sqrt => "sqrt",
    Exp => "exp",
    Exp2 => "exp2",
    Exp10 => "exp10",
    Log => "log",
    Log2 => "log2",
    Log10 => "log10",
    Modf => "modf",
    Floor => "floor",
    Ceil => "ceil",
    Trunc => "trunc",
    Rint => "rint",
    Nearbyint => "nearbyint",
    Round => "round",
    Roundeven => "roundeven",
    Lrint => "lrint",
    Llrint => "llrint",
    Fshl => "fshl",
    Fshr => "fshr",
    Frexp => "frexp",
    GetActiveLaneMask => "get.active.lane.mask",
    MatrixColumnMajorLoad => "matrix.column.major.load",
    MatrixColumnMajorStore => "matrix.column.major.store",
    Assume => "assume",
    Vscale => "vscale",
    LaunderInvariantGroup => "launder.invariant.group",
    Aarch64Irg => "aarch64.irg",
    Aarch64Tagp => "aarch64.tagp",
    AmdgcnMakeBufferRsrc => "amdgcn.make.buffer.rsrc",
    ThreadlocalAddress => "threadlocal.address",
    ExperimentalGuard => "experimental.guard",
    ExperimentalGetVectorLength => "experimental.get.vector.length",
    RiscvVsetvlimax => "riscv.vsetvlimax",
    Minnum => "minnum",
    Maxnum => "maxnum",
    Minimum => "minimum",
    Maximum => "maximum",
    Minimumnum => "minimumnum",
    Maximumnum => "maximumnum",
}

/// `Function::getIntrinsicID` of the function named `name` (`llvm.smax.i32`),
/// for the intrinsics of [`IntrinsicId`]: the longest name that is the callee's
/// before its type suffix.
pub fn intrinsic_id_of_name(name: &str) -> Option<IntrinsicId> {
    let name = name.strip_prefix("llvm.")?;
    INTRINSIC_NAMES
        .iter()
        .filter(|(base, _)| name.strip_prefix(base).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
        .max_by_key(|(base, _)| base.len())
        .map(|(_, id)| *id)
}

/// `IntrinsicInst::getIntrinsicID` of a call: `inst` calls one of
/// [`IntrinsicId`].
pub fn intrinsic_id(
    q: &Query,
    inst: InstId,
) -> Option<IntrinsicId> {
    intrinsic_id_of_name(q.callee_name(inst)?)
}
