//! InstSimplify's floating-point folds: `fneg`, `fadd`, `fsub`, `fmul`,
//! `fdiv`, `frem`, and the head of `fcmp`.
//!
//! Translation of `simplifyFNegInst` (:5923), `propagateNaN` (:5943),
//! `simplifyFPOp` (:5985), `simplifyFAddInst` (:6023), `simplifyFSubInst`
//! (:6090), `simplifyFMAFMul` (:6159), `simplifyFMulInst` (:6212),
//! `simplifyFDivInst` (:6256) and `simplifyFRemInst` (:6317) at d1106deb71cc
//! ([InstructionSimplify.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L5923)),
//! and the floating cases of `ConstantFoldBinaryInstruction`
//! ([ConstantFold.cpp:660](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L660)).
//!
//! MIRROR: the order of checks, each fast-math flag read from the instruction
//! (`FMF`), `foldOrCommuteConstant` first, the NaN/poison/undef rules of
//! `simplifyFPOp`.
//!
//! DEVIATE 1: the default floating environment only (`ebIgnore`, nearest
//! even): MIR has no constrained intrinsics, so `canIgnoreSNaN` is true and
//! `canRoundingModeBe(TowardNegative)` false.
//!
//! DEVIATE 2: `cannotBeNegativeZero` of a value (#2209) is answered for
//! constants alone; the `fadd X, 0.0` fold therefore finds less than LLVM,
//! never more.
//!
//! DEVIATE 3: `frem` of two constants is folded with the host's `fmod` (exact)
//! where neither is a NaN: `APFloat` has no `mod` here.
//!
//! An `x86_fp80` constant is its 80 bits (`fp80.rs`); `frem` of two is not
//! folded.
//!
//! `simplifyFCmpInst` is translated down to the constant, `x fcmp x`, `nnan`
//! and NaN-constant folds (DEVIATE 5, on `simplify_fcmp`).

use super::*;
use crate::apfloat::{ApFloat, IEEE_DOUBLE, IEEE_SINGLE, Sem};
use crate::fpclass::{FC_INF, FC_NAN, RoundingMode};
use crate::types::{FloatKind, Type};
use crate::valuetracking::fp::compute_known_fp_class;

const NEAREST_EVEN: RoundingMode = RoundingMode::NearestTiesToEven;

/// The constant's value.
fn value(
    q: &Query,
    operand: Operand,
) -> Option<ApFloat> {
    let constant = q.context.get(q.constant(operand)?);
    let Type::Float(kind) = *q.context.types.get(constant.ty) else { return None };
    if kind == FloatKind::X86Fp80 {
        return crate::fp80::value(&constant.kind);
    }
    let (sem, bits): (Sem, u64) = match (kind, &constant.kind) {
        (FloatKind::Float, ConstantKind::Float(bits)) => (IEEE_SINGLE, *bits),
        (_, ConstantKind::Float(bits)) => (IEEE_DOUBLE, *bits),
        (FloatKind::Float, ConstantKind::Null | ConstantKind::Zero) => (IEEE_SINGLE, 0),
        (_, ConstantKind::Null | ConstantKind::Zero) => (IEEE_DOUBLE, 0),
        _ => return None,
    };
    Some(ApFloat::from_bits(sem, u128::from(bits)))
}

fn sem_of(
    q: &Query,
    ty: TypeId,
) -> Sem {
    match q.context.types.get(ty) {
        Type::Float(FloatKind::Float) => IEEE_SINGLE,
        Type::Float(FloatKind::X86Fp80) => crate::apfloat::X87_DOUBLE_EXTENDED,
        _ => IEEE_DOUBLE,
    }
}

/// `PoisonValue::get`.
fn poison(
    q: &Query,
    ty: TypeId,
) -> Operand {
    Operand::Constant(q.context.constant(Constant { ty, kind: ConstantKind::Poison }))
}

/// `ConstantFP::get(Ty, V)`.
fn constant_of(
    q: &Query,
    ty: TypeId,
    value: &ApFloat,
) -> Operand {
    if matches!(q.context.types.get(ty), Type::Float(FloatKind::X86Fp80)) {
        return Operand::Constant(crate::fp80::constant(q.context, ty, value));
    }
    Operand::Constant(q.context.constant(Constant { ty, kind: ConstantKind::Float(value.to_bits() as u64) }))
}

/// `ConstantFP::getZero(Ty, Negative)`.
fn zero_of(
    q: &Query,
    ty: TypeId,
    negative: bool,
) -> Operand {
    constant_of(q, ty, &ApFloat::get_zero(sem_of(q, ty), negative))
}

fn is_nan(
    q: &Query,
    v: Operand,
) -> bool {
    value(q, v).is_some_and(|a| a.is_nan())
}

fn is_inf(
    q: &Query,
    v: Operand,
) -> bool {
    value(q, v).is_some_and(|a| a.is_infinity())
}

fn is_one(
    q: &Query,
    v: Operand,
) -> bool {
    value(q, v).is_some_and(|a| a.to_bits() == ApFloat::get_one(a.get_semantics(), false).to_bits())
}

fn is_any_zero(
    q: &Query,
    v: Operand,
) -> bool {
    value(q, v).is_some_and(|a| a.is_zero())
}

fn is_pos_zero(
    q: &Query,
    v: Operand,
) -> bool {
    value(q, v).is_some_and(|a| a.is_pos_zero())
}

fn is_neg_zero(
    q: &Query,
    v: Operand,
) -> bool {
    value(q, v).is_some_and(|a| a.is_neg_zero())
}

/// `m_FSub(m_AnyZeroFP(), m_Specific(x))`; with `m_Value(X)` the other side is
/// returned.
fn fsub_from_zero(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    let (_, instruction) = q.tracking().instruction_of(v)?;
    let [zero, x] = instruction.operands[..] else { return None };
    (instruction.opcode == Opcode::Binary(BinaryOp::FSub) && is_any_zero(q, zero)).then_some(x)
}

/// `m_FSub(m_Value(X), m_Value(Y))`.
fn fsub_of(
    q: &Query,
    v: Operand,
) -> Option<(Operand, Operand)> {
    let (_, instruction) = q.tracking().instruction_of(v)?;
    let [x, y] = instruction.operands[..] else { return None };
    (instruction.opcode == Opcode::Binary(BinaryOp::FSub)).then_some((x, y))
}

/// `m_c_FBinOp(m_Specific(y), m_Value(X))`: the other operand of `v`, an
/// `op` with `y` as an operand.
fn commutative_with(
    q: &Query,
    v: Operand,
    op: BinaryOp,
    y: Operand,
) -> Option<Operand> {
    let (_, instruction) = q.tracking().instruction_of(v)?;
    let [a, b] = instruction.operands[..] else { return None };
    if instruction.opcode != Opcode::Binary(op) {
        return None;
    }
    if a == y {
        Some(b)
    } else if b == y {
        Some(a)
    } else {
        None
    }
}

/// `m_FNeg(m_Value(X))`.
fn fneg_of(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    pattern::fneg(&q.tracking(), v)
}

/// `cannotBeNegativeZero(V, Q)`, for a constant (DEVIATE 2).
fn cannot_be_negative_zero(
    q: &Query,
    v: Operand,
) -> bool {
    value(q, v).is_some_and(|a| !a.is_neg_zero())
}

/// `ConstantFoldUnaryOpOperand(FNeg, ..)`.
fn fold_fneg(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    let ty = q.ty(v);
    if q.is_poison(v) {
        return Some(poison(q, ty));
    }
    if q.is_undef(v) {
        return Some(v);
    }
    Some(constant_of(q, ty, &value(q, v)?.neg()))
}

/// `propagateNaN`: an existing NaN quieted, else the canonical one.
fn propagate_nan(
    q: &Query,
    v: Operand,
) -> Operand {
    let ty = q.ty(v);
    match value(q, v) {
        Some(a) if a.is_nan() => constant_of(q, ty, &a.make_quiet()),
        _ => canonical_nan(q, ty),
    }
}

/// `ConstantFP::getNaN(Ty)`.
fn canonical_nan(
    q: &Query,
    ty: TypeId,
) -> Operand {
    constant_of(q, ty, &ApFloat::get_nan(sem_of(q, ty), false, 0))
}

/// `ConstantFoldBinaryInstruction` of an `FBinOp`: a poison operand, the
/// undef cases, then the `APFloat` operation.
fn fold_binary(
    q: &Query,
    op: BinaryOp,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let ty = q.ty(lhs);
    if q.constant(lhs).is_none() || q.constant(rhs).is_none() {
        return None;
    }
    let x87 = matches!(q.context.types.get(ty), Type::Float(FloatKind::X86Fp80));
    if q.is_poison(lhs) || q.is_poison(rhs) {
        return Some(poison(q, ty));
    }
    let undef = |v| q.constant(v).is_some_and(|id| q.context.get(id).kind == ConstantKind::Undef);
    if undef(lhs) || undef(rhs) {
        // -0.0 - undef --> undef (consistent with "fneg undef")
        if op == BinaryOp::FSub && is_neg_zero(q, lhs) && undef(rhs) {
            return Some(rhs);
        }
        // [any flop] undef, undef -> undef; otherwise NaN, which any operand
        // chosen for the undef propagates.
        return Some(if undef(lhs) && undef(rhs) { lhs } else { canonical_nan(q, ty) });
    }
    let (mut a, b) = (value(q, lhs)?, value(q, rhs)?);
    // DEVIATE 6: where the function evaluates at the x87's extended precision
    // (`FP_EVAL`), a double operation is rounded once, at the store, so only a
    // result the double holds exactly may be folded: 0.00005 * 10000 rounded
    // to 0.5 at 53 bits is a tie where the 64-bit product is above it.
    let extended = q
        .function
        .attrs
        .iter()
        .any(
            |attr| matches!(
                attr,
                crate::Attribute::Str(name, Some(value)) if name == crate::constantfolding::FP_EVAL && value == crate::constantfolding::FP_EVAL_EXTENDED
            ),
        );
    let status = match op {
        BinaryOp::FAdd => a.add(&b, NEAREST_EVEN),
        BinaryOp::FSub => a.subtract(&b, NEAREST_EVEN),
        BinaryOp::FMul => a.multiply(&b, NEAREST_EVEN),
        BinaryOp::FDiv => a.divide(&b, NEAREST_EVEN),
        _ => crate::apfloat::OP_OK,
    };
    // An x87 value is the format the hardware computes in: its rounding is the
    // program's.
    if extended && !x87 && status & crate::apfloat::OP_INEXACT != 0 {
        return None;
    }
    match op {
        BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv => {}
        BinaryOp::FRem => {
            if a.is_nan() || b.is_nan() || x87 {
                return None;
            }
            let bits = if a.get_semantics().size_in_bits == 32 {
                let (x, y) = (f32::from_bits(a.to_bits() as u32), f32::from_bits(b.to_bits() as u32));
                u128::from((x % y).to_bits())
            } else {
                let (x, y) = (f64::from_bits(a.to_bits() as u64), f64::from_bits(b.to_bits() as u64));
                u128::from((x % y).to_bits())
            };
            a = ApFloat::from_bits(a.get_semantics(), bits);
        }
        _ => return None,
    }
    Some(constant_of(q, ty, &a))
}

/// `foldOrCommuteConstant(Opcode, Op0, Op1, Q)`: the constants folded, else a
/// constant on the left of a commutative operation moved right (returned as
/// the swapped pair).
fn fold_or_commute(
    q: &Query,
    op: BinaryOp,
    lhs: Operand,
    rhs: Operand,
) -> Result<Operand, (Operand, Operand)> {
    if q.constant(lhs).is_some() {
        if q.constant(rhs).is_some() {
            return fold_binary(q, op, lhs, rhs).ok_or((lhs, rhs));
        }
        if matches!(op, BinaryOp::FAdd | BinaryOp::FMul) {
            return Err((rhs, lhs));
        }
    }
    Err((lhs, rhs))
}

/// `simplifyFPOp`: the folds common to every floating operation, which depend
/// on poison, undef and NaN operands alone.
fn simplify_fp_op(
    q: &Query,
    ops: [Operand; 2],
    flags: Flags,
) -> Option<Operand> {
    let ty = q.ty(ops[0]);
    if ops.iter().any(|&v| q.is_poison(v)) {
        return Some(poison(q, ty));
    }
    for v in ops {
        let (nan, inf, undef) = (is_nan(q, v), is_inf(q, v), q.is_undef(v));
        if flags.contains(Flags::NNAN) && (nan || undef) {
            return Some(poison(q, ty));
        }
        if flags.contains(Flags::NINF) && (inf || undef) {
            return Some(poison(q, ty));
        }
        // Undef propagates as a canonical NaN: its exponent bits would be
        // limited by a NaN operand anyway.
        if undef {
            return Some(canonical_nan(q, ty));
        }
        if nan {
            return Some(propagate_nan(q, v));
        }
    }
    None
}

/// `simplifyFNegInst`.
pub(super) fn simplify_fneg(
    q: &Query,
    op: Operand,
) -> Option<Operand> {
    if q.constant(op).is_some()
        && let Some(folded) = fold_fneg(q, op)
    {
        return Some(folded);
    }
    // fneg (fneg X) ==> X
    fneg_of(q, op)
}

/// `simplifyFCmpInst`
/// ([:4198](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L4198)):
/// two constants fold, a constant moves to the right, then the trivial
/// predicates, poison, an undef operand (a NaN, so true where the predicate
/// is unordered), `x fcmp x`, `ord`/`uno` under `nnan`, and a NaN on the
/// right. DEVIATE 5: the folds that read `computeKnownFPClass` of a value
/// (`ord`/`uno` of two values, `fcmpToClassTest`, the compares against a
/// negative or zero constant), `isImpliedByDomCondition` (the integer
/// predicates only), `min/maxnum` with a constant, and the select and phi
/// threading are not translated; a vector compare is left alone.
pub(super) fn simplify_fcmp(
    q: &Query,
    pred: FloatPredicate,
    lhs: Operand,
    rhs: Operand,
    flags: Flags,
) -> Option<Operand> {
    let Some(Type::Float(_)) = q.function.operand_type(q.context, lhs).map(|ty| q.context.types.get(ty)) else {
        return None;
    };
    let (mut pred, mut lhs, mut rhs) = (pred, lhs, rhs);
    if let Some(left) = q.constant(lhs) {
        if let Some(right) = q.constant(rhs) {
            if let Some(folded) = constfold::compare_float(q.context, pred, left, right) {
                return Some(Operand::Constant(folded));
            }
        } else {
            // A constant goes on the right.
            std::mem::swap(&mut lhs, &mut rhs);
            pred = pred.swapped();
        }
    }
    let i1 = q.context.types.int(1);
    let answer = |held: bool| {
        Operand::Constant(q.context.constant(Constant { ty: i1, kind: ConstantKind::Int(u128::from(held)) }))
    };
    // Fold trivial predicates.
    if matches!(pred, FloatPredicate::False | FloatPredicate::True) {
        return Some(answer(pred == FloatPredicate::True));
    }
    if q.is_poison(lhs) || q.is_poison(rhs) {
        return Some(poison(q, i1));
    }
    if q.is_undef(lhs) || q.is_undef(rhs) {
        return Some(answer(pred.holds(None)));
    }
    // `CmpInst::isTrueWhenEqual` and `isFalseWhenEqual`: not every predicate
    // is decided by `x` against itself (a NaN is unordered).
    if lhs == rhs {
        match pred {
            FloatPredicate::Ueq | FloatPredicate::Uge | FloatPredicate::Ule => return Some(answer(true)),
            FloatPredicate::One | FloatPredicate::Ogt | FloatPredicate::Olt => return Some(answer(false)),
            _ => {}
        }
    }
    if matches!(pred, FloatPredicate::Ord | FloatPredicate::Uno) && flags.contains(Flags::NNAN) {
        return Some(answer(pred == FloatPredicate::Ord));
    }
    // `C->isNaN()`.
    if value(q, rhs).is_some_and(|c| c.is_nan()) {
        return Some(answer(pred.holds(None)));
    }
    None
}

/// `simplifyFAddInst`.
fn simplify_fadd(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
    flags: Flags,
) -> Option<Operand> {
    let (op0, op1) = match fold_or_commute(q, BinaryOp::FAdd, lhs, rhs) {
        Ok(folded) => return Some(folded),
        Err(pair) => pair,
    };
    if let Some(c) = simplify_fp_op(q, [op0, op1], flags) {
        return Some(c);
    }
    let nsz = flags.contains(Flags::NSZ);
    // fadd X, -0 ==> X
    if is_neg_zero(q, op1) {
        return Some(op0);
    }
    // fadd X, 0 ==> X, when we know X is not -0
    if is_pos_zero(q, op1) && (nsz || cannot_be_negative_zero(q, op0)) {
        return Some(op0);
    }
    if flags.contains(Flags::NNAN) {
        // With nnan: X + {+/-}Inf --> {+/-}Inf
        if is_inf(q, op1) {
            return Some(op1);
        }
        // With nnan: -X + X --> 0.0 (and commuted variant). Infinities need
        // no exclusion (INF + -INF is NaN), nor do negative zeros (the sum is
        // +0.0).
        let zero_minus = |a, b| fsub_from_zero(q, a) == Some(b);
        if zero_minus(op0, op1) || zero_minus(op1, op0) || fneg_of(q, op0) == Some(op1) || fneg_of(q, op1) == Some(op0)
        {
            return Some(zero_of(q, q.ty(op0), false));
        }
    }
    // (X - Y) + Y --> X
    // Y + (X - Y) --> X
    if nsz && flags.contains(Flags::REASSOC) {
        for (sub, y) in [(op0, op1), (op1, op0)] {
            if let Some((x, sub_y)) = fsub_of(q, sub)
                && sub_y == y
            {
                return Some(x);
            }
        }
    }
    None
}

/// `simplifyFSubInst`.
fn simplify_fsub(
    q: &Query,
    op0: Operand,
    op1: Operand,
    flags: Flags,
) -> Option<Operand> {
    if let Ok(folded) = fold_or_commute(q, BinaryOp::FSub, op0, op1) {
        return Some(folded);
    }
    if let Some(c) = simplify_fp_op(q, [op0, op1], flags) {
        return Some(c);
    }
    let nsz = flags.contains(Flags::NSZ);
    // fsub X, +0 ==> X
    if is_pos_zero(q, op1) {
        return Some(op0);
    }
    // fsub X, -0 ==> X, when we know X is not -0
    if is_neg_zero(q, op1) && (nsz || cannot_be_negative_zero(q, op0)) {
        return Some(op0);
    }
    // fsub -0.0, (fsub -0.0, X) ==> X
    // fsub -0.0, (fneg X) ==> X
    if is_neg_zero(q, op0)
        && let Some(x) = fneg_of(q, op1)
    {
        return Some(x);
    }
    // fsub 0.0, (fsub 0.0, X) ==> X if signed zeros are ignored.
    // fsub 0.0, (fneg X) ==> X if signed zeros are ignored.
    if nsz
        && is_any_zero(q, op0)
        && let Some(x) = fsub_from_zero(q, op1).or_else(|| fneg_of(q, op1))
    {
        return Some(x);
    }
    if flags.contains(Flags::NNAN) {
        // fsub nnan x, x ==> 0.0
        if op0 == op1 {
            return Some(zero_of(q, q.ty(op0), false));
        }
        // With nnan: {+/-}Inf - X --> {+/-}Inf
        if is_inf(q, op0) {
            return Some(op0);
        }
        // With nnan: X - {+/-}Inf --> {-/+}Inf
        if is_inf(q, op1) {
            return fold_fneg(q, op1);
        }
    }
    // Y - (Y - X) --> X
    // (X + Y) - Y --> X
    if nsz && flags.contains(Flags::REASSOC) {
        if let Some((y, x)) = fsub_of(q, op1)
            && y == op0
        {
            return Some(x);
        }
        if let Some(x) = commutative_with(q, op0, BinaryOp::FAdd, op1) {
            return Some(x);
        }
    }
    None
}

/// `simplifyFMAFMul`.
fn simplify_fma_fmul(
    q: &Query,
    op0: Operand,
    op1: Operand,
    flags: Flags,
) -> Option<Operand> {
    if let Some(c) = simplify_fp_op(q, [op0, op1], flags) {
        return Some(c);
    }
    // Canonicalize special constants as operand 1.
    let (op0, op1) = if is_one(q, op0) || is_any_zero(q, op0) { (op1, op0) } else { (op0, op1) };
    // X * 1.0 --> X
    if is_one(q, op1) {
        return Some(op0);
    }
    if is_any_zero(q, op1) {
        // X * 0.0 --> 0.0 (with nnan and nsz)
        if flags.contains(Flags::NNAN) && flags.contains(Flags::NSZ) {
            return Some(zero_of(q, q.ty(op0), false));
        }
        let known = compute_known_fp_class(&q.tracking(), op0, FC_INF | FC_NAN, 0);
        if known.is_known_never_inf_or_nan() {
            if flags.contains(Flags::NSZ) {
                return Some(zero_of(q, q.ty(op0), false));
            }
            // +normal number * (-)0.0 --> (-)0.0
            // -normal number * (-)0.0 --> -(-)0.0
            match known.get_sign_bit() {
                Some(false) => return Some(op1),
                Some(true) => return fold_fneg(q, op1),
                None => {}
            }
        }
    }
    // sqrt(X) * sqrt(X) --> X, if the intermediate rounding may go (reassoc),
    // sqrt of a negative is NaN (nnan), and sqrt(-0.0) is -0.0 but -0.0 *
    // -0.0 is 0.0 (nsz).
    if op0 == op1
        && flags.contains(Flags::REASSOC)
        && flags.contains(Flags::NNAN)
        && flags.contains(Flags::NSZ)
        && let Some((_, [x])) = pattern::intrinsic_call(&q.tracking(), op0, "llvm.sqrt")
    {
        return Some(*x);
    }
    None
}

/// `simplifyFMulInst`.
fn simplify_fmul(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
    flags: Flags,
) -> Option<Operand> {
    let (op0, op1) = match fold_or_commute(q, BinaryOp::FMul, lhs, rhs) {
        Ok(folded) => return Some(folded),
        Err(pair) => pair,
    };
    simplify_fma_fmul(q, op0, op1, flags)
}

/// `simplifyFDivInst`.
fn simplify_fdiv(
    q: &Query,
    op0: Operand,
    op1: Operand,
    flags: Flags,
) -> Option<Operand> {
    if let Ok(folded) = fold_or_commute(q, BinaryOp::FDiv, op0, op1) {
        return Some(folded);
    }
    if let Some(c) = simplify_fp_op(q, [op0, op1], flags) {
        return Some(c);
    }
    // X / 1.0 -> X
    if is_one(q, op1) {
        return Some(op0);
    }
    let (nnan, nsz) = (flags.contains(Flags::NNAN), flags.contains(Flags::NSZ));
    let ty = q.ty(op0);
    // 0 / X -> 0, with NaNs off (X could be zero) and signed zeroes ignored
    // (X could be either sign).
    if nnan && nsz && is_any_zero(q, op0) {
        return Some(zero_of(q, ty, false));
    }
    if nnan {
        // X / X -> 1.0, infinities aside: INF / INF is NaN.
        if op0 == op1 {
            return Some(constant_of(q, ty, &ApFloat::get_one(sem_of(q, ty), false)));
        }
        // (X * Y) / Y --> X if we can reassociate to the above form.
        if flags.contains(Flags::REASSOC)
            && let Some(x) = commutative_with(q, op0, BinaryOp::FMul, op1)
        {
            return Some(x);
        }
        // -X / X -> -1.0 and X / -X -> -1.0, signed zeros aside: +-0.0/+-0.0
        // is NaN.
        if fsub_from_zero(q, op0) == Some(op1) || fsub_from_zero(q, op1) == Some(op0) {
            return Some(constant_of(q, ty, &ApFloat::get_one(sem_of(q, ty), true)));
        }
        // nnan ninf X / [-]0.0 -> poison
        if flags.contains(Flags::NINF) && is_any_zero(q, op1) {
            return Some(poison(q, q.ty(op1)));
        }
    }
    None
}

/// `simplifyFRemInst`.
fn simplify_frem(
    q: &Query,
    op0: Operand,
    op1: Operand,
    flags: Flags,
) -> Option<Operand> {
    if let Ok(folded) = fold_or_commute(q, BinaryOp::FRem, op0, op1) {
        return Some(folded);
    }
    if let Some(c) = simplify_fp_op(q, [op0, op1], flags) {
        return Some(c);
    }
    // The result of frem has the sign of the dividend.
    if flags.contains(Flags::NNAN) {
        // +0 % X -> 0
        if is_pos_zero(q, op0) {
            return Some(zero_of(q, q.ty(op0), false));
        }
        // -0 % X -> -0
        if is_neg_zero(q, op0) {
            return Some(zero_of(q, q.ty(op0), true));
        }
    }
    None
}

/// The floating binary operations of `simplifyBinOp`.
pub(super) fn simplify_fp_binop(
    q: &Query,
    op: BinaryOp,
    lhs: Operand,
    rhs: Operand,
    flags: Flags,
) -> Option<Operand> {
    match op {
        BinaryOp::FAdd => simplify_fadd(q, lhs, rhs, flags),
        BinaryOp::FSub => simplify_fsub(q, lhs, rhs, flags),
        BinaryOp::FMul => simplify_fmul(q, lhs, rhs, flags),
        BinaryOp::FDiv => simplify_fdiv(q, lhs, rhs, flags),
        BinaryOp::FRem => simplify_frem(q, lhs, rhs, flags),
        _ => None,
    }
}

#[cfg(test)]
#[path = "simplify_fp_tests.rs"]
mod tests;
