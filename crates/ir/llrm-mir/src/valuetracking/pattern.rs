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
use crate::constantrange::{ConstantRange, PreferredRangeType};
use crate::context::ConstantKind;
use crate::module::{BlockId, Function, InstId, Instruction, Operand, ValueDef};
use crate::opcode::{Attribute, BinaryOp, CallInfo, Opcode};

/// `m_BinOp`-family: `v` is `op a, b`: its instruction and both operands.
pub fn binary<'a>(
    q: &Query<'a>,
    v: Operand,
    op: BinaryOp,
) -> Option<(InstId, &'a Instruction, Operand, Operand)> {
    let (inst, instruction) = q.instruction_of(v)?;
    (matches!(instruction.opcode, Opcode::Binary(kind) if kind == op))
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
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
    (matches!(instruction.opcode, Opcode::Cast(kind) if kind == op))
        .then(|| (inst, instruction, instruction.operands[0]))
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
    if matches!(instruction.opcode, Opcode::Binary(BinaryOp::And)) {
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
    if matches!(instruction.opcode, Opcode::Binary(BinaryOp::Or)) {
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

        pub(crate) const INTRINSIC_NAMES: &[(&str, IntrinsicId)] = &[$(($name, IntrinsicId::$variant),)*];
    };
}

intrinsics! {
    SaddWithOverflow => "sadd.with.overflow",
    SsubWithOverflow => "ssub.with.overflow",
    SmulWithOverflow => "smul.with.overflow",
    UaddWithOverflow => "uadd.with.overflow",
    UsubWithOverflow => "usub.with.overflow",
    UmulWithOverflow => "umul.with.overflow",
    Fabs => "fabs",
    Fma => "fma",
    Fmuladd => "fmuladd",
    Ldexp => "ldexp",
    ArithmeticFence => "arithmetic.fence",
    FptruncRound => "fptrunc.round",
    Copysign => "copysign",
    Ctpop => "ctpop",
    Ctlz => "ctlz",
    Cttz => "cttz",
    Memset => "memset",
    MemsetPattern => "experimental.memset.pattern",
    Memcpy => "memcpy",
    MemcpyInline => "memcpy.inline",
    Memmove => "memmove",
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
    SdivFix => "sdiv.fix",
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
    Clmul => "clmul",
    Pdep => "pdep",
    Pext => "pext",
    Smulh => "smulh",
    Umulh => "umulh",
    X86Sse42Crc32_64_64 => "x86.sse42.crc32.64.64",
    AmdgcnMbcntHi => "amdgcn.mbcnt.hi",
    AmdgcnMbcntLo => "amdgcn.mbcnt.lo",
    RiscvVsetvli => "riscv.vsetvli",
}

/// `Function::getIntrinsicID` of the function named `name` (`llvm.smax.i32`),
/// for the intrinsics of [`IntrinsicId`]: the longest name that is the callee's
/// before its type suffix.
pub fn intrinsic_id_of_name(name: &str) -> Option<IntrinsicId> {
    static BY_BASE: std::sync::OnceLock<crate::hash::HashMap<&'static str, IntrinsicId>> = std::sync::OnceLock::new();
    let name = name.strip_prefix("llvm.")?;
    let by_base = BY_BASE.get_or_init(|| INTRINSIC_NAMES.iter().copied().collect());
    // A base is the whole name or what stands before a `.`: the longest one
    // found wins.
    let mut end = name.len();
    loop {
        #[cfg(test)]
        NAME_PROBES.with(|one| one.set(one.get() + 1));
        if let Some(&id) = by_base.get(&name[..end]) {
            return Some(id);
        }
        end = name[..end].rfind('.')?;
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static NAME_PROBES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `IntrinsicInst::getIntrinsicID` of a call: `inst` calls one of
/// [`IntrinsicId`].
pub fn intrinsic_id(
    q: &Query,
    inst: InstId,
) -> Option<IntrinsicId> {
    q.callee_intrinsic_id(inst)
}

// ---- nonzero.rs ----

/// `dyn_cast<IntrinsicInst>(I)->getIntrinsicID()` for a call (not an invoke).
pub fn call_intrinsic(
    q: &Query,
    inst: InstId,
) -> Option<IntrinsicId> {
    matches!(q.function.instruction(inst).opcode, Opcode::Call(_)).then_some(())?;
    intrinsic_id(q, inst)
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
    matches!(
        apint(q, v),
        Some((bits, width)) if bits == crate::context::mask(width)
    )
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

/// `m_c_ICmp(Pred, m_Specific(x), m_Value(Y))`: the predicate with `x` on the
/// left, and `Y`.
pub fn c_icmp_specific(
    q: &Query,
    v: Operand,
    x: Operand,
) -> Option<(IntPredicate, Operand)> {
    let (_, _, CmpPredicate { pred, .. }, lhs, rhs) = icmp(q, v)?;
    if lhs == x {
        Some((pred, rhs))
    } else if rhs == x {
        Some((pred.swapped(), lhs))
    } else {
        None
    }
}

// ---- known.rs ----

/// Matchers that nest: LLVM's `m_*` objects as closures over one operand,
/// for `known.rs`, which binds sub-matches as it goes.
pub mod nested {
    use std::cell::Cell;

    use super::super::query::Query;
    use super::{all_ones, zero_int};
    use crate::module::Operand;
    use crate::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};

    /// A matcher: LLVM's `m_*` object, called on one operand.
    pub type Matcher<'a> = &'a dyn Fn(Operand) -> bool;

    /// `m_Value()`.
    pub fn any(_: Operand) -> bool {
        true
    }

    /// `m_Value(X)`: matches anything and remembers it.
    pub fn bind(cell: &Cell<Option<Operand>>) -> impl Fn(Operand) -> bool + '_ {
        move |v| {
            cell.set(Some(v));
            true
        }
    }

    /// `m_Specific(X)` and `m_Deferred(X)`: the value `cell` holds when asked.
    pub fn specific(cell: &Cell<Option<Operand>>) -> impl Fn(Operand) -> bool + '_ {
        move |v| cell.get() == Some(v)
    }

    /// `m_Specific(V)`.
    pub fn exactly(value: Operand) -> impl Fn(Operand) -> bool {
        move |v| v == value
    }

    /// `m_APInt(C)`, remembering the constant in `cell`.
    pub fn bind_apint<'a>(
        q: &'a Query,
        cell: &'a Cell<Option<(u128, u32)>>,
    ) -> impl Fn(Operand) -> bool + 'a {
        move |v| {
            let found = q.int_constant(v);
            if found.is_some() {
                cell.set(found);
            }
            found.is_some()
        }
    }

    /// `m_SpecificInt(Value)`: a constant integer of that unsigned value.
    pub fn specific_int(
        q: &Query,
        v: Operand,
        value: u128,
    ) -> bool {
        q.int_constant(v).is_some_and(|(bits, _)| bits == value)
    }

    /// `BinaryOp_match<L, R, Opcode, Commutable>` on a binary operator with the
    /// flags `needs`: left is tried first, on the first operand, then, if
    /// `commutable`, on the second.
    pub fn match_binary(
        q: &Query,
        v: Operand,
        op: BinaryOp,
        needs: Flags,
        commutable: bool,
        l: Matcher,
        r: Matcher,
    ) -> bool {
        let Some((_, instruction)) = q.instruction_of(v) else { return false };
        if !matches!(instruction.opcode, Opcode::Binary(kind) if kind == op) || !instruction.flags.contains(needs) {
            return false;
        }
        let (a, b) = (instruction.operands[0], instruction.operands[1]);
        (l(a) && r(b)) || (commutable && l(b) && r(a))
    }

    /// `AnyBinaryOp_match<L, R, Commutable>`: the operator's opcode as well.
    pub fn match_any_binary(
        q: &Query,
        v: Operand,
        commutable: bool,
        l: Matcher,
        r: Matcher,
    ) -> Option<BinaryOp> {
        let (_, instruction) = q.instruction_of(v)?;
        let Opcode::Binary(op) = instruction.opcode else { return None };
        let (a, b) = (instruction.operands[0], instruction.operands[1]);
        ((l(a) && r(b)) || (commutable && l(b) && r(a))).then_some(op)
    }

    /// `m_Not(V)`: `xor V, -1` in either order.
    pub fn not(
        q: &Query,
        v: Operand,
        inner: Matcher,
    ) -> bool {
        match_binary(q, v, BinaryOp::Xor, Flags::default(), true, &|x| all_ones(q, x), inner)
    }

    /// `m_Neg(V)`: `sub 0, V`.
    pub fn neg(
        q: &Query,
        v: Operand,
        inner: Matcher,
    ) -> bool {
        match_binary(q, v, BinaryOp::Sub, Flags::default(), false, &|x| zero_int(q, x), inner)
    }

    /// `m_ZExt(V)`, `m_SExt(V)`, `m_Trunc(V)`: the cast and its operand
    /// matching.
    pub fn cast(
        q: &Query,
        v: Operand,
        op: CastOp,
        inner: Matcher,
    ) -> bool {
        let Some((_, instruction)) = q.instruction_of(v) else { return false };
        matches!(instruction.opcode, Opcode::Cast(kind) if kind == op) && inner(instruction.operands[0])
    }

    /// `m_ICmp(Pred, L, R)` with `m_c_ICmp`'s swap when `commutable`: the
    /// predicate as matched.
    pub fn match_icmp(
        q: &Query,
        v: Operand,
        commutable: bool,
        l: Matcher,
        r: Matcher,
    ) -> Option<IntPredicate> {
        let (_, instruction) = q.instruction_of(v)?;
        let Opcode::ICmp(pred) = instruction.opcode else { return None };
        let (a, b) = (instruction.operands[0], instruction.operands[1]);
        if l(a) && r(b) {
            return Some(pred);
        }
        (commutable && l(b) && r(a)).then(|| pred.swapped())
    }
}

/// `CallBase::arg_size()`: a call's operands are its arguments and its callee,
/// an invoke's also its two destinations.
pub fn arg_size(instruction: &Instruction) -> usize {
    let extra = if matches!(instruction.opcode, Opcode::Invoke(_)) { 3 } else { 1 };
    instruction.operands.len().saturating_sub(extra)
}

/// A call's arguments: its operands but the callee (and, of an invoke, its
/// two destinations).
pub fn call_arguments(instruction: &Instruction) -> &[Operand] {
    &instruction.operands[..arg_size(instruction)]
}

/// `CallBase::getCalledFunction()`.
pub fn called_function<'a>(
    q: &Query<'a>,
    instruction: &Instruction,
) -> Option<&'a Function> {
    let (Opcode::Call(_) | Opcode::Invoke(_)) = instruction.opcode else { return None };
    let Operand::Constant(id) = *instruction.operands.last()? else { return None };
    let ConstantKind::Global(global) = q.context().get(id).kind else { return None };
    q.globals.get(global.0 as usize)?.function()
}

pub fn call_info(instruction: &Instruction) -> Option<&CallInfo> {
    match &instruction.opcode {
        Opcode::Call(info) | Opcode::Invoke(info) => Some(info),
        _ => None,
    }
}

pub fn range_attribute(
    q: &Query,
    attrs: &[Attribute],
) -> Option<ConstantRange> {
    attrs
        .iter()
        .find_map(
            |attr| match attr {
                Attribute::Range { ty, lower, upper } => {
                    Some(ConstantRange::new(q.context().types.int_bits(*ty)?, *lower, *upper))
                }
                _ => None,
            },
        )
}

/// `Argument::getRange()`: the `range` attribute of parameter `index`.
pub fn argument_range(
    q: &Query,
    index: u32,
) -> Option<ConstantRange> {
    range_attribute(q, q.function.parameter_attrs.get(index as usize)?)
}

/// `CallBase::getRange()`: the call's `range` return attribute, intersected
/// with the callee's.
pub fn call_range(
    q: &Query,
    instruction: &Instruction,
) -> Option<ConstantRange> {
    let call = call_info(instruction)?;
    let at_call = range_attribute(q, &call.return_attrs);
    let at_function = called_function(q, instruction).and_then(|function| range_attribute(q, &function.return_attrs));
    match (at_call, at_function) {
        (Some(one), Some(other)) => Some(one.intersect_with(&other, PreferredRangeType::Smallest)),
        (one, other) => one.or(other),
    }
}

/// `CallBase::getReturnedArgOperand()`.
pub fn returned_argument_operand(
    q: &Query,
    instruction: &Instruction,
) -> Option<Operand> {
    let returned = |attrs: &Vec<Vec<Attribute>>| {
        attrs.iter().position(|one| one.iter().any(|attr| matches!(attr, Attribute::Flag(name) if name == "returned")))
    };
    let call = call_info(instruction)?;
    let index = returned(&call.argument_attrs)
        .or_else(|| called_function(q, instruction).and_then(|function| returned(&function.parameter_attrs)))?;
    call_arguments(instruction).get(index).copied()
}

/// `CallBase::getRetAlign()`.
pub fn call_return_align(
    q: &Query,
    instruction: &Instruction,
) -> Option<u64> {
    let align = |attrs: &[Attribute]| {
        attrs
            .iter()
            .find_map(
                |attr| match attr {
                    Attribute::Int(name, value) if name == "align" => Some(*value),
                    _ => None,
                },
            )
    };
    let call = call_info(instruction)?;
    align(&call.return_attrs)
        .or_else(|| called_function(q, instruction).and_then(|function| align(&function.return_attrs)))
}

/// The incoming values and blocks of a phi.
pub fn phi_incoming(instruction: &Instruction) -> impl Iterator<Item = (Operand, BlockId)> + '_ {
    instruction
        .operands
        .chunks(2)
        .map(
            |pair| {
                let Operand::Block(from) = pair[1] else { unreachable!("a phi's block") };
                (pair[0], from)
            },
        )
}

/// The result of `inst` as an operand.
pub fn result_of(instruction: &Instruction) -> Operand {
    Operand::Value(instruction.result.expect("an instruction with a result"))
}

/// The instruction that defines `v` when it is an instruction of class phi.
pub fn phi_of<'a>(
    q: &Query<'a>,
    v: Operand,
) -> Option<(InstId, &'a Instruction)> {
    q.instruction_of(v).filter(|(_, instruction)| instruction.opcode == Opcode::Phi)
}

/// `m_c_BinOp(m_Phi(P), m_Value())` on `bo`: the phi that is one of its
/// operands.
pub fn phi_operand_of<'a>(
    q: &Query<'a>,
    bo: &Instruction,
) -> Option<(InstId, &'a Instruction)> {
    phi_of(q, bo.operands[0]).or_else(|| phi_of(q, bo.operands[1]))
}

/// `isa<IntrinsicInst>(I)`: a call to a function named `llvm.*`.
pub fn is_intrinsic(
    q: &Query,
    inst: InstId,
) -> bool {
    matches!(q.function.instruction(inst).opcode, Opcode::Call(_))
        && q.callee_name(inst).is_some_and(|name| name.starts_with("llvm."))
}

/// `matchTwoInputRecurrence<InstTy>(PN, Inst, Init, OtherOp)`: `accept` says
/// whether an incoming value is an `InstTy`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9695>
pub fn match_two_input_recurrence<'a>(
    q: &Query<'a>,
    phi: InstId,
    accept: &dyn Fn(InstId, &Instruction) -> bool,
) -> Option<(InstId, Operand, Operand)> {
    let instruction = q.function.instruction(phi);
    if instruction.operands.len() != 4 {
        return None;
    }
    let this = result_of(instruction);
    let incoming: Vec<Operand> = phi_incoming(instruction).map(|(value, _)| value).collect();
    for i in 0..2 {
        let Some((inst, operation)) = q.instruction_of(incoming[i]) else { continue };
        if !accept(inst, operation) || operation.operands.len() < 2 {
            continue;
        }
        let (lhs, rhs) = (operation.operands[0], operation.operands[1]);
        if lhs != this && rhs != this {
            continue;
        }
        return Some((inst, incoming[1 - i], if lhs == this { rhs } else { lhs }));
    }
    None
}

/// `matchThreeInputRecurrence<InstTy>(PN, Inst, Init, OtherOp0, OtherOp1)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9722>
pub fn match_three_input_recurrence<'a>(
    q: &Query<'a>,
    phi: InstId,
    accept: &dyn Fn(InstId, &Instruction) -> bool,
) -> Option<(InstId, Operand, Operand, Operand)> {
    let instruction = q.function.instruction(phi);
    if instruction.operands.len() != 4 {
        return None;
    }
    let this = result_of(instruction);
    let incoming: Vec<Operand> = phi_incoming(instruction).map(|(value, _)| value).collect();
    for i in 0..2 {
        let Some((inst, operation)) = q.instruction_of(incoming[i]) else { continue };
        if !accept(inst, operation) || operation.operands.len() < 3 {
            continue;
        }
        let (op0, op1, op2) = (operation.operands[0], operation.operands[1], operation.operands[2]);
        if op0 != this && op1 != this && op2 != this {
            continue;
        }
        let (other0, other1) = if op0 == this {
            (op1, op2)
        } else if op1 == this {
            (op0, op2)
        } else {
            (op0, op1)
        };
        return Some((inst, incoming[1 - i], other0, other1));
    }
    None
}

/// `matchSimpleRecurrence(P, BO, Start, Step)`: `%iv = phi [Start], [%iv.next]`
/// with `%iv.next = binop %iv, Step` or `binop Step, %iv`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9755>
pub fn match_simple_recurrence<'a>(
    q: &Query<'a>,
    phi: InstId,
) -> Option<(InstId, Operand, Operand)> {
    match_two_input_recurrence(q, phi, &|_, operation| matches!(operation.opcode, Opcode::Binary(_)))
}

/// `matchSimpleRecurrence(I, P, Start, Step)`: the phi `bo` is the
/// recurrence of, and its start and step.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9766>
pub fn match_simple_recurrence_of<'a>(
    q: &Query<'a>,
    bo: InstId,
) -> Option<(InstId, Operand, Operand)> {
    let instruction = q.function.instruction(bo);
    if !matches!(instruction.opcode, Opcode::Binary(_)) {
        return None;
    }
    let (phi, _) = phi_operand_of(q, instruction)?;
    let (found, start, step) = match_simple_recurrence(q, phi)?;
    (found == bo).then_some((phi, start, step))
}

/// `matchSimpleBinaryIntrinsicRecurrence(I, P, Init, OtherOp)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9773>
pub fn match_simple_binary_intrinsic_recurrence<'a>(
    q: &Query<'a>,
    inst: InstId,
) -> Option<(InstId, Operand, Operand)> {
    let instruction = q.function.instruction(inst);
    let arguments = call_arguments(instruction);
    let ty = instruction.ty;
    if arguments.len() != 2 || q.type_of(arguments[0]) != Some(ty) || q.type_of(arguments[1]) != Some(ty) {
        return None;
    }
    let phi = phi_of(q, arguments[0]).or_else(|| phi_of(q, arguments[1]))?.0;
    let (found, init, other) = match_two_input_recurrence(q, phi, &|inst, _| is_intrinsic(q, inst))?;
    (found == inst).then_some((phi, init, other))
}

/// `matchSimpleTernaryIntrinsicRecurrence(I, P, Init, OtherOp0, OtherOp1)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9789>
pub fn match_simple_ternary_intrinsic_recurrence<'a>(
    q: &Query<'a>,
    inst: InstId,
) -> Option<(InstId, Operand, Operand, Operand)> {
    let instruction = q.function.instruction(inst);
    let arguments = call_arguments(instruction);
    let ty = instruction.ty;
    if arguments.len() != 3 || arguments.iter().any(|argument| q.type_of(*argument) != Some(ty)) {
        return None;
    }
    let phi = phi_of(q, arguments[0]).or_else(|| phi_of(q, arguments[1])).or_else(|| phi_of(q, arguments[2]))?.0;
    let (found, init, other0, other1) = match_three_input_recurrence(q, phi, &|inst, _| is_intrinsic(q, inst))?;
    (found == inst).then_some((phi, init, other0, other1))
}

/// Whether `v` is an argument (`isa<Argument>`).
pub fn is_argument(
    q: &Query,
    v: Operand,
) -> bool {
    matches!(
        v,
        Operand::Value(id) if matches!(q.function.value(id).def, ValueDef::Argument(_))
    )
}

// ---- combine_shared.rs ----

/// `m_Neg(m_Value(X))`: `sub 0, x`, with the flags of the `sub`.
pub fn neg(
    q: &Query,
    v: Operand,
) -> Option<(InstId, Operand, Flags)> {
    let (inst, instruction, a, b) = binary(q, v, BinaryOp::Sub)?;
    zero(q, a).then_some((inst, b, instruction.flags))
}
