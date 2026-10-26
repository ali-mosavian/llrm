//! `computeKnownFPClass`: which classes of floating-point value a value can be.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! `computeKnownFPClass`
//! ([L5214](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L5214)),
//! each case linking its line. The operations on the facts are
//! [`KnownFpClass`]'s. The result is what the value is known never to be: the
//! instruction's `nnan`/`ninf` and the cases below rule classes out.
//! `interested` says which classes the caller asks about; an operand is
//! inspected only when an answer in them could change.
//!
//! Slice 1 of the analysis: constants, the fast-math flags, `fneg`, `fabs`,
//! `copysign`, `select`, `phi`, `fadd`, `fsub`, `fmul`, `fdiv`, `frem`,
//! `fpext`, `fptrunc`, `sitofp` and `uitofp`.
//!
//! - MIRROR 1: the order of the cases, the recursion limit and the phi limit
//!   (`MaxAnalysisRecursionDepth - 2`), `interested` narrowed by the flags.
//! - DEVIATE 1: scalars only. A vector value, and the vector cases (constant
//!   vectors and data sequences, `extractelement`, `insertelement`,
//!   `shufflevector`, `extractvalue`) are unknown.
//! - DEVIATE 2: `computeKnownFPClassFromContext` reads no `Q.CC` beyond the
//!   query's condition context and no `DomConditionCache` unless the query has
//!   one, as the pin does; the exponent-range query
//!   (`computeKnownExponentRangeFromContext`) is not translated.
//! - DEVIATE 3: a function's `denormal-fp-math` is not read; every function is
//!   IEEE (`DenormalMode::getIEEE`), as `getDenormalMode` answers for a
//!   function with no attribute.
//! - DEVIATE 4: no `nofpclass` on an argument, a call result or a load's
//!   `!nofpclass`; an `x86_fp80` constant is held as the nearest double, so it
//!   is unknown. The `fma` recurrence of a phi
//!   (`matchSimpleTernaryIntrinsicRecurrence`) is not translated.
//! - DEVIATE 5: no `experimental.constrained.*`, `amdgcn.*`, `vector.reduce.*`,
//!   `vector.reverse` or `convert.from.arbitrary.fp`, and no `ppc_fp128`
//!   (`isMultiUnitFPType` is false): MIR has none of them.
//! - DEVIATE 6: `m_c_MaxOrMin` of a `bitcast` finds the `select` form through
//!   `matchSelectPattern`, where the C++ matches the compare in place.
//!
//! COVERED: the oracle (`tests/vt/fp`, `tools/llvmport/fpdump.sh`, with and
//! without the `DomConditionCache`) matches LLVM on all 2,298 float and double
//! values of the 70 files of `Transforms/InstSimplify` and
//! `Transforms/InstCombine` that llrm reads. The other 249 of the 319 files
//! that mention a floating operation do not parse (#3180): 66 use vectors,
//! deferred until MIR has vector types (not forbidden: nothing here assumes a
//! scalar but the `Type::Float` test at the entry), the rest are reader gaps or
//! types outside MIR's subset (`half`, `fp128`).
//!
//! # Example
//! `uitofp i32 %x to float` is never a NaN, a negative or a subnormal.
//!
//! # Used by
//! Not yet: the folds that read it follow.

use super::fpredicate;
use super::pattern::{self, IntrinsicId};
use super::query::Query;
use super::{MAX_ANALYSIS_RECURSION_DEPTH, context, guarantee, known, range};
use crate::apfloat::{ApFloat, IEEE_DOUBLE, IEEE_SINGLE, Sem, semantics_of};
use crate::constantrange::ConstantRange;
use crate::context::ConstantKind;
use crate::context::{mask, signed};
use crate::fpclass::{
    DenormalMode, FC_ALL_FLAGS, FC_FINITE, FC_INF, FC_NAN, FC_NEG_FINITE, FC_NEG_INF, FC_NEG_NORMAL, FC_NEG_SUBNORMAL,
    FC_NEG_ZERO, FC_NEGATIVE, FC_NONE, FC_NORMAL, FC_POS_FINITE, FC_POS_INF, FC_POS_NORMAL, FC_POS_SUBNORMAL,
    FC_POS_ZERO, FC_POSITIVE, FC_SUBNORMAL, FC_ZERO, FpClassTest,
};
use crate::knownfpclass::{KnownFpClass, MinMaxKind};
use crate::module::{InstId, Instruction, Operand};
use crate::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use crate::types::{FloatKind, Type};

/// `computeKnownFPClass(V, InterestedClasses, Q, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L6476>
pub fn compute_known_fp_class(
    q: &Query,
    v: Operand,
    interested: FpClassTest,
    depth: u32,
) -> KnownFpClass {
    let mut known = KnownFpClass::default();
    compute_into(q, v, interested, &mut known, depth);
    known
}

/// The float kind of a scalar value's type.
fn float_kind(
    q: &Query,
    v: Operand,
) -> Option<FloatKind> {
    match q.type_of(v).map(|ty| q.context().types.get(ty)) {
        Some(Type::Float(kind)) => Some(*kind),
        _ => None,
    }
}

fn sem_of(
    q: &Query,
    v: Operand,
) -> Option<Sem> {
    float_kind(q, v).map(semantics_of)
}

/// `Mode`: every function is IEEE (DEVIATE 3).
fn denormal_mode() -> DenormalMode {
    DenormalMode::get_ieee()
}

/// The floating constant `v` is, an `x86_fp80` one excepted (DEVIATE 4).
fn constant_value(
    q: &Query,
    v: Operand,
) -> Option<ApFloat> {
    let Operand::Constant(id) = v else { return None };
    let constant = q.context().get(id);
    let Type::Float(kind) = *q.context().types.get(constant.ty) else { return None };
    let sem = match kind {
        FloatKind::Float => IEEE_SINGLE,
        FloatKind::Double => IEEE_DOUBLE,
        FloatKind::X86Fp80 => return None,
    };
    match constant.kind {
        ConstantKind::Float(bits) => Some(ApFloat::from_bits(sem, u128::from(bits))),
        ConstantKind::Null | ConstantKind::Zero => Some(ApFloat::from_bits(sem, 0)),
        _ => None,
    }
}

/// `isAbsoluteValueULEOne`: `fabs(x) <= 1.0` or NaN, for `x - floor(x)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L5197>
fn is_absolute_value_ule_one(
    q: &Query,
    v: Operand,
) -> bool {
    let Some((_, sub)) = q.instruction_of(v) else { return false };
    if sub.opcode != Opcode::Binary(BinaryOp::FSub) {
        return false;
    }
    pattern::intrinsic(q, sub.operands[1], IntrinsicId::Floor)
        .is_some_and(|(_, floor)| pattern::call_arguments(floor)[0] == sub.operands[0])
}

fn compute_into(
    q: &Query,
    v: Operand,
    mut interested: FpClassTest,
    known: &mut KnownFpClass,
    depth: u32,
) {
    // DEVIATE 1.
    let Some(kind) = float_kind(q, v) else { return };
    if let Some(constant) = constant_value(q, v) {
        *known = KnownFpClass::from_apfloat(&constant);
        return;
    }
    if let Operand::Constant(id) = v {
        if q.context().get(id).kind == ConstantKind::Poison {
            known.set_known_fp_classes(FC_NONE);
            known.set_sign_bit(Some(false));
        }
        return;
    }
    let operator = q.instruction_of(v);

    // The classes the flags rule out.
    let mut not_from_flags = FC_NONE;
    if let Some((_, instruction)) = operator {
        if instruction.flags.contains(Flags::NNAN) {
            not_from_flags |= FC_NAN;
        }
        if instruction.flags.contains(Flags::NINF) {
            not_from_flags |= FC_INF;
        }
    }
    let assumed = known_from_context(q, v);
    not_from_flags |= !assumed.get_known_fp_classes();
    // We no longer need to find out about these bits from inputs if we can
    // assume this from flags.
    interested &= !not_from_flags;

    // All recursive calls that increase depth must come after this.
    if let Some((inst, instruction)) = operator
        && depth < MAX_ANALYSIS_RECURSION_DEPTH
    {
        of_operator(q, inst, instruction, kind, interested, not_from_flags, known, depth);
    }
    known.known_not(not_from_flags);
    if known.get_sign_bit().is_none()
        && let Some(sign) = assumed.get_sign_bit()
    {
        if sign {
            known.sign_bit_must_be_one();
        } else {
            known.sign_bit_must_be_zero();
        }
    }
}

/// `isSignBitCheck`
/// ([L4947](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4947)):
/// whether `icmp pred x, rhs` (`rhs` of `width` bits) only checks the sign
/// bit, and if so whether it is true for a negative `x`.
pub fn is_sign_bit_check(
    pred: IntPredicate,
    rhs: u128,
    width: u32,
) -> Option<bool> {
    let all_ones = mask(width);
    let max_signed = all_ones >> 1;
    let min_signed = max_signed + 1;
    match pred {
        // True if LHS s< 0.
        IntPredicate::Slt => (rhs == 0).then_some(true),
        // True if LHS s<= -1.
        IntPredicate::Sle => (rhs == all_ones).then_some(true),
        // True if LHS s> -1.
        IntPredicate::Sgt => (rhs == all_ones).then_some(false),
        // True if LHS s>= 0.
        IntPredicate::Sge => (rhs == 0).then_some(false),
        // True if LHS u> RHS and RHS == sign-bit-mask - 1.
        IntPredicate::Ugt => (rhs == max_signed).then_some(true),
        // True if LHS u>= RHS and RHS == sign-bit-mask.
        IntPredicate::Uge => (rhs == min_signed).then_some(true),
        // True if LHS u< RHS and RHS == sign-bit-mask.
        IntPredicate::Ult => (rhs == min_signed).then_some(false),
        // True if LHS u<= RHS and RHS == sign-bit-mask - 1.
        IntPredicate::Ule => (rhs == max_signed).then_some(false),
        _ => None,
    }
}

/// `computeKnownFPClassFromCond`
/// ([L4971](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4971)).
fn known_from_cond(
    q: &Query,
    v: Operand,
    cond: Operand,
    cond_is_true: bool,
    known: &mut KnownFpClass,
    depth: u32,
) {
    if depth < MAX_ANALYSIS_RECURSION_DEPTH {
        let pair = if cond_is_true { pattern::logical_and(q, cond) } else { pattern::logical_or(q, cond) };
        if let Some((a, b)) = pair {
            known_from_cond(q, v, a, cond_is_true, known, depth + 1);
            known_from_cond(q, v, b, cond_is_true, known, depth + 1);
            return;
        }
        if let Some(a) = pattern::not(q, cond) {
            known_from_cond(q, v, a, !cond_is_true, known, depth + 1);
            return;
        }
    }
    let Some((_, instruction)) = q.instruction_of(cond) else { return };
    match &instruction.opcode {
        Opcode::FCmp(pred) => {
            let lhs = instruction.operands[0];
            let Some(crhs) = fpredicate::match_constant_float(q, instruction.operands[1]) else { return };
            let implied = fpredicate::fcmp_implies_class_of(q, *pred, lhs, &crhs, lhs != v);
            if implied.value == Some(v) {
                known.known_not(!(if cond_is_true { implied.if_true } else { implied.if_false }));
            }
        }
        Opcode::ICmp(pred) => {
            // `m_ICmp(Pred, m_ElementWiseBitCast(m_Specific(V)),
            // m_APInt(RHS))`.
            let Some((_, cast)) = q.instruction_of(instruction.operands[0]) else { return };
            if cast.opcode != Opcode::Cast(CastOp::BitCast) || cast.operands[0] != v {
                return;
            }
            let Some((rhs, width)) = q.int_constant(instruction.operands[1]) else { return };
            let Some(true_if_signed) = is_sign_bit_check(*pred, rhs, width) else { return };
            if true_if_signed == cond_is_true {
                known.sign_bit_must_be_one();
            } else {
                known.sign_bit_must_be_zero();
            }
        }
        Opcode::Call(_) => {
            let Some((inst, call)) = q.instruction_of(cond) else { return };
            let _ = inst;
            let arguments = pattern::call_arguments(call);
            if pattern::intrinsic(q, cond, IntrinsicId::IsFpclass).is_some()
                && arguments[0] == v
                && let Some((class, _)) = q.int_constant(arguments[1])
            {
                let class = FpClassTest(class as u32);
                known.known_not(if cond_is_true { !class } else { class });
            }
        }
        _ => {}
    }
}

/// `computeKnownFPClassFromContext`
/// ([L5079](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L5079)).
fn known_from_context(
    q: &Query,
    v: Operand,
) -> KnownFpClass {
    let mut known = KnownFpClass::default();
    if let Some(cc) = q.cond_context
        && cc.affected_values.contains(&v)
    {
        known_from_cond(q, v, cc.cond, !cc.invert, &mut known, 0);
    }
    let Some(cxt) = q.cxt else { return known };
    if let (Some(conditions), Some(_)) = (q.dom_conditions, q.dt) {
        let cxt_block = q.block_of(cxt);
        for &branch in conditions.conditions_for(v).iter() {
            let instruction = q.function.instruction(branch);
            let (Some(start), Some(cxt_block), Operand::Block(taken), Operand::Block(otherwise)) =
                (q.block_of(branch), cxt_block, instruction.operands[1], instruction.operands[2])
            else {
                continue;
            };
            if context::edge_dominates(q, start, taken, cxt_block) {
                known_from_cond(q, v, instruction.operands[0], true, &mut known, 0);
            }
            if context::edge_dominates(q, start, otherwise, cxt_block) {
                known_from_cond(q, v, instruction.operands[0], false, &mut known, 0);
            }
        }
    }
    let Some(assumptions) = q.assumptions else { return known };
    // Try to restrict the floating-point classes based on information from
    // assumptions.
    for &assume in assumptions.assumptions_for(v) {
        if !context::is_valid_assume_for_context(q, assume, cxt, false) {
            continue;
        }
        known_from_cond(q, v, q.function.instruction(assume).operands[0], true, &mut known, 0);
    }
    known
}

/// `adjustKnownFPClassForSelectArm`
/// ([L5128](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L5128)).
fn adjust_for_select_arm(
    q: &Query,
    known: &mut KnownFpClass,
    cond: Operand,
    arm: Operand,
    invert: bool,
    depth: u32,
) {
    let mut known_src = KnownFpClass::default();
    known_from_cond(q, arm, cond, !invert, &mut known_src, depth + 1);
    known_src = known_src.union_with(known);
    if known_src.is_unknown() {
        return;
    }
    if guarantee::is_guaranteed_not_to_be_undef(q, arm, depth + 1) {
        *known = known_src;
    }
}

#[allow(clippy::too_many_arguments)]
fn of_operator(
    q: &Query,
    inst: InstId,
    op: &Instruction,
    kind: FloatKind,
    interested: FpClassTest,
    not_from_flags: FpClassTest,
    known: &mut KnownFpClass,
    depth: u32,
) {
    let again = |operand: Operand, classes: FpClassTest| compute_known_fp_class(q, operand, classes, depth + 1);
    match &op.opcode {
        // L5342
        Opcode::FNeg => {
            *known = again(op.operands[0], interested);
            known.fneg();
        }
        // L5352
        Opcode::Select => {
            let for_arm = |arm: Operand, invert: bool| {
                let mut res = again(arm, interested);
                adjust_for_select_arm(q, &mut res, op.operands[0], arm, invert, depth);
                res
            };
            // Only known if known in both the LHS and RHS.
            *known = for_arm(op.operands[1], false).intersect_with(&for_arm(op.operands[2], true));
        }
        Opcode::Call(_) => of_call(q, inst, op, kind, interested, not_from_flags, known, depth),
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub) => fadd_fsub(q, op, interested, known, depth),
        Opcode::Binary(BinaryOp::FMul) => fmul(q, op, known, depth),
        Opcode::Binary(BinaryOp::FDiv) => fdiv(q, op, interested, known, depth),
        Opcode::Binary(BinaryOp::FRem) => frem(q, op, interested, known, depth),
        Opcode::Cast(CastOp::FPExt) => {
            let src = again(op.operands[0], interested);
            let (Some(dst_sem), Some(src_sem)) = (Some(semantics_of(kind)), sem_of(q, op.operands[0])) else { return };
            *known = KnownFpClass::fpext(&src, dst_sem, src_sem);
        }
        // L6176
        Opcode::Cast(CastOp::FPTrunc) => fptrunc(q, op, interested, known, depth),
        Opcode::Cast(cast @ (CastOp::SIToFP | CastOp::UIToFP)) => {
            int_to_fp(q, op, *cast == CastOp::SIToFP, kind, interested, known, depth);
        }
        Opcode::Phi => phi(q, inst, op, interested, known, depth),
        // L6438
        Opcode::Cast(CastOp::BitCast) => bitcast(q, op, kind, interested, known, depth),
        _ => {}
    }
}

/// `computeKnownFPClassForFPTrunc`
/// ([L5162](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L5162)).
fn fptrunc(
    q: &Query,
    op: &Instruction,
    interested: FpClassTest,
    known: &mut KnownFpClass,
    depth: u32,
) {
    if (interested & (KnownFpClass::ORDERED_LESS_THAN_ZERO_MASK | FC_NAN)) == FC_NONE {
        return;
    }
    *known = KnownFpClass::fptrunc(&compute_known_fp_class(q, op.operands[0], interested, depth + 1));
}

/// `getMinMaxKind`
/// ([L5178](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L5178)).
fn min_max_kind(id: IntrinsicId) -> Option<MinMaxKind> {
    Some(match id {
        IntrinsicId::Minimum => MinMaxKind::Minimum,
        IntrinsicId::Maximum => MinMaxKind::Maximum,
        IntrinsicId::Minimumnum => MinMaxKind::Minimumnum,
        IntrinsicId::Maximumnum => MinMaxKind::Maximumnum,
        IntrinsicId::Minnum => MinMaxKind::Minnum,
        IntrinsicId::Maxnum => MinMaxKind::Maxnum,
        _ => return None,
    })
}

/// The intrinsics of `case Instruction::Call`
/// ([L5300](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L5300)).
/// DEVIATE 5: no `experimental.constrained.*`, `amdgcn.*`, `vector.reduce.*`,
/// `vector.reverse` or `convert.from.arbitrary.fp`: MIR has none of them.
#[allow(clippy::too_many_arguments)]
fn of_call(
    q: &Query,
    inst: InstId,
    op: &Instruction,
    kind: FloatKind,
    interested: FpClassTest,
    not_from_flags: FpClassTest,
    known: &mut KnownFpClass,
    depth: u32,
) {
    let again = |operand: Operand, classes: FpClassTest| compute_known_fp_class(q, operand, classes, depth + 1);
    let arguments = pattern::call_arguments(op);
    let Some(id) = pattern::call_intrinsic(q, inst) else { return };
    let mode = denormal_mode();
    // `computeKnownFPClass(Arg0, InterestedClasses)` then `f(KnownSrc)`.
    let unary = |f: fn(&KnownFpClass) -> KnownFpClass| f(&again(arguments[0], interested));
    match id {
        // L5393
        IntrinsicId::Fabs => {
            if (interested & (FC_NAN | FC_POSITIVE)) != FC_NONE {
                // If we only care about the sign bit we don't need to inspect
                // the operand.
                *known = again(arguments[0], interested);
            }
            known.fabs();
        }
        // L5403
        IntrinsicId::Copysign => {
            *known = again(arguments[0], interested);
            let sign = again(arguments[1], interested);
            known.copysign(&sign);
        }
        // L5413
        IntrinsicId::Fma | IntrinsicId::Fmuladd => {
            if (interested & FC_NEGATIVE) == FC_NONE {
                return;
            }
            // FIXME (as in the pin): this should check
            // isGuaranteedNotToBeUndef.
            if arguments[0] == arguments[1] {
                let mut src = again(arguments[0], interested);
                let mut addend = again(arguments[2], interested);
                if (not_from_flags & FC_NAN) != FC_NONE {
                    src.known_not(FC_NAN);
                    addend.known_not(FC_NAN);
                }
                if (not_from_flags & FC_INF) != FC_NONE {
                    src.known_not(FC_INF);
                    addend.known_not(FC_INF);
                }
                *known = KnownFpClass::fma_square(&src, &addend, mode);
                return;
            }
            let mut srcs = [KnownFpClass::default(); 3];
            for (at, src) in srcs.iter_mut().enumerate() {
                *src = again(arguments[at], interested);
                if src.is_unknown() {
                    return;
                }
                if (not_from_flags & FC_NAN) != FC_NONE {
                    src.known_not(FC_NAN);
                }
                if (not_from_flags & FC_INF) != FC_NONE {
                    src.known_not(FC_INF);
                }
            }
            *known = KnownFpClass::fma(&srcs[0], &srcs[1], &srcs[2], mode);
        }
        // L5457
        IntrinsicId::Sqrt => {
            let mut interested_srcs = interested;
            if (interested & FC_NAN) != FC_NONE {
                interested_srcs |= KnownFpClass::ORDERED_LESS_THAN_ZERO_MASK;
            }
            let src = again(arguments[0], interested_srcs);
            let has_nsz = op.flags.contains(Flags::NSZ);
            // `DenormalMode::getDynamic()` where the call has `nsz`.
            let mode = if has_nsz { DenormalMode::get_dynamic() } else { mode };
            *known = KnownFpClass::sqrt(&src, mode);
            if has_nsz {
                known.known_not(FC_NEG_ZERO);
            }
        }
        IntrinsicId::Sin => *known = unary(KnownFpClass::sin),
        IntrinsicId::Cos => *known = unary(KnownFpClass::cos),
        IntrinsicId::Tan => *known = unary(KnownFpClass::tan),
        IntrinsicId::Sinh => *known = unary(KnownFpClass::sinh),
        IntrinsicId::Cosh => *known = unary(KnownFpClass::cosh),
        IntrinsicId::Tanh => *known = unary(KnownFpClass::tanh),
        IntrinsicId::Asin => *known = unary(KnownFpClass::asin),
        IntrinsicId::Acos => *known = unary(KnownFpClass::acos),
        IntrinsicId::Atan => *known = unary(KnownFpClass::atan),
        // L5546
        IntrinsicId::Atan2 => {
            let mut interested_y = interested;
            let mut interested_x = interested;
            // We can rule out negative values if y cannot have a negative
            // value.
            if (interested & FC_NEG_FINITE) != FC_NONE {
                interested_y |= FC_NEGATIVE;
            }
            // We can rule out positive values if y cannot have a positive
            // value.
            if (interested & FC_POS_FINITE) != FC_NONE {
                interested_y |= FC_POSITIVE | FC_NEG_SUBNORMAL;
            }
            // We can rule out zero and subnormal if x cannot have a positive
            // value.
            if (interested & (FC_ZERO | FC_SUBNORMAL)) != FC_NONE {
                interested_x |= FC_POSITIVE | FC_NEG_SUBNORMAL;
            }
            let known_y = again(arguments[0], interested_y);
            let known_x = again(arguments[1], interested_x);
            *known = KnownFpClass::atan2(&known_y, &known_x, mode);
        }
        // L5576
        IntrinsicId::Maxnum
        | IntrinsicId::Minnum
        | IntrinsicId::Minimum
        | IntrinsicId::Maximum
        | IntrinsicId::Minimumnum
        | IntrinsicId::Maximumnum => {
            let known_lhs = again(arguments[0], interested);
            let known_rhs = again(arguments[1], interested);
            let kind = min_max_kind(id).expect("a min/max");
            *known = KnownFpClass::min_max_like(&known_lhs, &known_rhs, kind, mode);
        }
        // L5599
        IntrinsicId::Canonicalize => {
            let src = again(arguments[0], interested);
            *known = KnownFpClass::canonicalize(&src, mode);
        }
        // L5633
        IntrinsicId::Trunc
        | IntrinsicId::Floor
        | IntrinsicId::Ceil
        | IntrinsicId::Rint
        | IntrinsicId::Nearbyint
        | IntrinsicId::Round
        | IntrinsicId::Roundeven => {
            let mut interested_srcs = interested;
            if (interested_srcs & FC_POS_FINITE) != FC_NONE {
                interested_srcs |= FC_POS_FINITE;
            }
            if (interested_srcs & FC_NEG_FINITE) != FC_NONE {
                interested_srcs |= FC_NEG_FINITE;
            }
            let src = again(arguments[0], interested_srcs);
            // No `ppc_fp128`: no scalar type of MIR is a multi-unit float.
            *known = KnownFpClass::round_to_integral(&src, id == IntrinsicId::Trunc, false);
        }
        // L5654
        IntrinsicId::Exp | IntrinsicId::Exp2 | IntrinsicId::Exp10 => *known = unary(KnownFpClass::exp),
        // L5670
        IntrinsicId::FptruncRound => fptrunc(q, op, interested, known, depth),
        // L5675
        IntrinsicId::Log | IntrinsicId::Log10 | IntrinsicId::Log2 => {
            let mut interested_srcs = FC_NONE;
            // log(negative) produces NaN.
            if (interested & FC_NAN) != FC_NONE {
                interested_srcs |= FC_NAN | FC_NEGATIVE;
            }
            // log(logical-zero) produces negative infinity.
            if (interested & FC_NEG_INF) != FC_NONE {
                interested_srcs |= FC_ZERO | FC_SUBNORMAL;
            }
            // log(x) < -0.0 if x < +1.0
            if (interested & FC_NEG_NORMAL) != FC_NONE {
                interested_srcs |= FC_POS_SUBNORMAL | FC_POS_NORMAL;
            }
            // log(x) >= +0.0 if x >= +1.0
            if (interested & (FC_POS_ZERO | FC_POS_NORMAL)) != FC_NONE {
                interested_srcs |= FC_POS_NORMAL;
            }
            // log(x) is positive infinity iff x is positive infinity.
            if (interested & FC_POS_INF) != FC_NONE {
                interested_srcs |= FC_POS_INF;
            }
            let src =
                if interested_srcs != FC_NONE { again(arguments[0], interested_srcs) } else { KnownFpClass::default() };
            *known = KnownFpClass::log(&src, mode);
        }
        // L5716
        IntrinsicId::Pow => {
            let want_nan = (interested & FC_NAN) != FC_NONE;
            let want_negative = (interested & FC_NEGATIVE) != FC_NONE;
            if !want_nan && !want_negative {
                return;
            }
            let mut interested_lhs = FC_NONE;
            let mut interested_rhs = FC_NONE;
            if want_nan {
                // pow may return NaN if one of the arguments is NaN. NaN may
                // also be produced from a negative, non-zero finite base and a
                // non-integer exponent.
                interested_lhs |= FC_NAN | FC_NEG_NORMAL | FC_NEG_SUBNORMAL;
                interested_rhs |= FC_NAN;
            }
            if want_negative {
                // A negative value is returned when a negative base is raised
                // to an odd integer power. Only normal values can be odd
                // integers.
                interested_lhs |= FC_NEGATIVE;
                interested_rhs |= FC_NORMAL;
            }
            let known_lhs = again(arguments[0], interested_lhs);
            // If the LHS is unknown, then querying the RHS is only useful for
            // rare edge cases.
            if known_lhs.is_unknown() {
                return;
            }
            let known_rhs = again(arguments[1], interested_rhs);
            *known = KnownFpClass::pow(&known_lhs, &known_rhs);
        }
        // L5753
        IntrinsicId::Powi => {
            if (interested & (FC_NAN | FC_INF | FC_NEGATIVE)) == FC_NONE {
                return;
            }
            // The exponent is always a scalar, even when raising a vector to a
            // power.
            let exponent = known::compute_known_bits(q, arguments[1], depth + 1);
            let mut interested_srcs = FC_NONE;
            if (interested & FC_NAN) != FC_NONE {
                interested_srcs |= FC_NAN;
            }
            if !exponent.is_zero() {
                if (interested & FC_INF) != FC_NONE {
                    interested_srcs |= FC_FINITE | FC_INF;
                }
                if (interested & FC_NEGATIVE) != FC_NONE && !exponent.is_even() {
                    interested_srcs |= FC_NEGATIVE;
                }
            }
            let src =
                if interested_srcs != FC_NONE { again(arguments[0], interested_srcs) } else { KnownFpClass::default() };
            *known = KnownFpClass::powi(&src, &exponent);
        }
        // L5781
        IntrinsicId::Ldexp => {
            let src = again(arguments[0], interested);
            // Can refine inf/zero handling based on the exponent operand.
            let exp_info_mask = FC_ZERO | FC_SUBNORMAL | FC_INF;
            let exp_width = q.scalar_bits(arguments[1]);
            let exp_range = if (src.get_known_fp_classes() & exp_info_mask) != FC_NONE {
                range::compute_constant_range(q, arguments[1], true, depth + 1)
            } else {
                ConstantRange::full(exp_width)
            };
            *known = KnownFpClass::ldexp(
                &src,
                signed(exp_range.get_signed_min(), exp_width),
                signed(exp_range.get_signed_max(), exp_width),
                semantics_of(kind),
                mode,
            );
        }
        // L5806
        IntrinsicId::ArithmeticFence => *known = again(arguments[0], interested),
        _ => {}
    }
}

/// `FAdd`, `FSub`
/// ([L5962](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L5962)).
fn fadd_fsub(
    q: &Query,
    op: &Instruction,
    interested: FpClassTest,
    known: &mut KnownFpClass,
    depth: u32,
) {
    let is_fadd = op.opcode == Opcode::Binary(BinaryOp::FAdd);
    let want_negative = is_fadd && (interested & KnownFpClass::ORDERED_LESS_THAN_ZERO_MASK) != FC_NONE;
    let want_nan = (interested & FC_NAN) != FC_NONE;
    let want_neg_zero = (interested & FC_NEG_ZERO) != FC_NONE;
    if !want_nan && !want_negative && !want_neg_zero {
        return;
    }
    let mut interested_srcs = interested;
    if want_negative {
        interested_srcs |= KnownFpClass::ORDERED_LESS_THAN_ZERO_MASK;
    }
    if (interested & FC_NAN) != FC_NONE {
        interested_srcs |= FC_INF;
    }
    let (lhs, rhs) = (op.operands[0], op.operands[1]);
    let known_rhs = compute_known_fp_class(q, rhs, interested_srcs, depth + 1);

    // Special case fadd x, x, which is the canonical form of fmul x, 2.
    let is_self = lhs == rhs && guarantee::is_guaranteed_not_to_be_undef(q, lhs, depth + 1);
    let mut known_lhs = if is_self { known_rhs } else { KnownFpClass::default() };

    if (want_nan && known_rhs.is_known_never_nan())
        || (want_negative && known_rhs.cannot_be_ordered_less_than_zero())
        || want_neg_zero
        || !is_fadd
    {
        let mode = denormal_mode();
        if is_self && is_fadd {
            *known = KnownFpClass::fadd_self(&known_lhs, mode);
        } else {
            // RHS is canonically cheaper to compute. Skip inspecting the LHS
            // if there's no point.
            if !is_self {
                known_lhs = compute_known_fp_class(q, lhs, interested_srcs, depth + 1);
            }
            *known = if is_fadd {
                KnownFpClass::fadd(&known_lhs, &known_rhs, mode)
            } else {
                KnownFpClass::fsub(&known_lhs, &known_rhs, mode)
            };
        }
    }
}

/// `FMul`
/// ([L6019](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L6019)).
fn fmul(
    q: &Query,
    op: &Instruction,
    known: &mut KnownFpClass,
    depth: u32,
) {
    let mode = denormal_mode();
    let (lhs, rhs) = (op.operands[0], op.operands[1]);
    // X * X is always non-negative or a NaN.
    // FIXME (as in the pin): should check isGuaranteedNotToBeUndef.
    if lhs == rhs {
        let src = compute_known_fp_class(q, lhs, FC_ALL_FLAGS, depth + 1);
        *known = KnownFpClass::square(&src, mode);
        return;
    }
    let known_lhs;
    let mut known_rhs = KnownFpClass::default();
    if let Some(crhs) = constant_value(q, rhs) {
        known_lhs = compute_known_fp_class(q, lhs, FC_ALL_FLAGS, depth + 1);
        *known = KnownFpClass::fmul_apfloat(&known_lhs, &crhs, mode);
    } else {
        known_rhs = compute_known_fp_class(q, rhs, FC_ALL_FLAGS, depth + 1);
        known_lhs = compute_known_fp_class(q, lhs, FC_ALL_FLAGS, depth + 1);
        *known = KnownFpClass::fmul(&known_lhs, &known_rhs, mode);
    }
    // Propagate no-infs if the other source is known smaller than one, such
    // that this cannot introduce overflow.
    if known_lhs.is_known_never(FC_INF) && is_absolute_value_ule_one(q, rhs)
        || known_rhs.is_known_never(FC_INF) && is_absolute_value_ule_one(q, lhs)
    {
        known.known_not(FC_INF);
    }
}

/// `FDiv`
/// ([L6066](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L6066)).
fn fdiv(
    q: &Query,
    op: &Instruction,
    interested: FpClassTest,
    known: &mut KnownFpClass,
    depth: u32,
) {
    let want_nan = (interested & FC_NAN) != FC_NONE;
    let mode = denormal_mode();
    let (lhs, rhs) = (op.operands[0], op.operands[1]);
    if lhs == rhs && guarantee::is_guaranteed_not_to_be_undef(q, lhs, 0) {
        // X / X is always exactly 1.0 or a NaN.
        known.set_known_fp_classes(FC_NAN | FC_POS_NORMAL);
        if !want_nan {
            return;
        }
        let src = compute_known_fp_class(q, lhs, FC_NAN | FC_INF | FC_ZERO | FC_SUBNORMAL, depth + 1);
        *known = KnownFpClass::fdiv_self(&src, mode);
        return;
    }
    let want_negative = (interested & FC_NEGATIVE) != FC_NONE;
    let want_positive = (interested & FC_POSITIVE) != FC_NONE;
    if !want_nan && !want_negative && !want_positive {
        return;
    }
    let known_rhs = compute_known_fp_class(q, rhs, FC_ALL_FLAGS, depth + 1);
    let useful = known_rhs.is_known_never_nan()
        || known_rhs.is_known_never(FC_NEG_NORMAL | FC_NEG_SUBNORMAL)
        || known_rhs.is_known_never(FC_POS_NORMAL | FC_POS_SUBNORMAL);
    let known_lhs =
        if useful { compute_known_fp_class(q, lhs, FC_ALL_FLAGS, depth + 1) } else { KnownFpClass::default() };
    *known = KnownFpClass::fdiv(&known_lhs, &known_rhs, mode);
}

/// `FRem`
/// ([L6113](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L6113)).
fn frem(
    q: &Query,
    op: &Instruction,
    interested: FpClassTest,
    known: &mut KnownFpClass,
    depth: u32,
) {
    let want_nan = (interested & FC_NAN) != FC_NONE;
    known.known_not(FC_INF);
    let mode = denormal_mode();
    let (lhs, rhs) = (op.operands[0], op.operands[1]);
    if lhs == rhs && guarantee::is_guaranteed_not_to_be_undef(q, lhs, 0) {
        // X % X is always exactly [+-]0.0 or a NaN.
        known.set_known_fp_classes(FC_NAN | FC_ZERO);
        if !want_nan {
            return;
        }
        let src = compute_known_fp_class(q, lhs, FC_NAN | FC_INF | FC_ZERO | FC_SUBNORMAL, depth + 1);
        *known = KnownFpClass::frem_self(&src, mode);
        return;
    }
    let want_negative = (interested & FC_NEGATIVE) != FC_NONE;
    let want_positive = (interested & FC_POSITIVE) != FC_NONE;
    if !want_nan && !want_negative && !want_positive {
        return;
    }
    let known_rhs = compute_known_fp_class(q, rhs, FC_NAN | FC_INF | FC_ZERO | FC_NEGATIVE, depth + 1);
    let useful = known_rhs.is_known_never_nan()
        || known_rhs.is_known_never(FC_NEGATIVE)
        || known_rhs.is_known_never(FC_POSITIVE);
    let known_lhs = if useful || want_positive {
        compute_known_fp_class(q, lhs, FC_ALL_FLAGS, depth + 1)
    } else {
        KnownFpClass::default()
    };
    *known = KnownFpClass::frem(&known_lhs, &known_rhs, mode);
}

/// `SIToFP`, `UIToFP`
/// ([L6181](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L6181)).
fn int_to_fp(
    q: &Query,
    op: &Instruction,
    signed: bool,
    kind: FloatKind,
    interested: FpClassTest,
    known: &mut KnownFpClass,
    depth: u32,
) {
    // Cannot produce nan.
    known.known_not(FC_NAN);
    // Integers cannot be subnormal.
    known.known_not(FC_SUBNORMAL);
    // sitofp and uitofp turn into +0.0 for zero.
    known.known_not(FC_NEG_ZERO);
    // UIToFP is always non-negative regardless of known bits.
    if !signed {
        known.sign_bit_must_be_zero();
    }
    // Only compute known bits if we can learn something useful from them.
    if (interested & (FC_POS_ZERO | FC_NORMAL | FC_INF)) == FC_NONE {
        return;
    }
    let int_known = known::compute_known_bits(q, op.operands[0], depth + 1);
    // If the integer is non-zero, the result cannot be +0.0.
    if int_known.is_non_zero() {
        known.known_not(FC_POS_ZERO);
    }
    if signed {
        // If the signed integer is known non-negative, the result is
        // non-negative. If the signed integer is known negative, the result is
        // negative.
        if int_known.is_non_negative() {
            known.sign_bit_must_be_zero();
        } else if int_known.is_negative() {
            known.sign_bit_must_be_one();
        }
    }
    // Guard kept for ilogb().
    if (interested & FC_INF) != FC_NONE {
        // Get width of largest magnitude integer known. This still works for a
        // signed minimum value because the largest FP value is scaled by some
        // fraction close to 2.0 (1.0 + 0.xxxx).
        let mut int_size = i64::from(int_known.width);
        if signed {
            int_size -= i64::from(int_known.count_min_sign_bits());
        } else {
            int_size -= i64::from(int_known.count_min_leading_zeros());
        }
        // If the exponent of the largest finite FP value can hold the largest
        // integer, the result of the cast must be finite.
        if i64::from(ApFloat::get_largest(semantics_of(kind), false).ilogb()) >= int_size {
            known.known_not(FC_INF);
        }
    }
}

/// `m_c_MaxOrMin(L, R)`: an integer `smax`, `smin`, `umax` or `umin` of two
/// values, as the intrinsic or as a `select` of a compare (found by
/// `matchSelectPattern`: DEVIATE 6).
fn max_or_min_operands(
    q: &Query,
    v: Operand,
) -> Option<(Operand, Operand)> {
    for id in [IntrinsicId::Smax, IntrinsicId::Smin, IntrinsicId::Umax, IntrinsicId::Umin] {
        if let Some((_, call)) = pattern::intrinsic(q, v, id) {
            let arguments = pattern::call_arguments(call);
            return Some((arguments[0], arguments[1]));
        }
    }
    let result = range::match_select_pattern(q, v);
    if !matches!(
        result.flavor,
        range::SelectPatternFlavor::SMin
            | range::SelectPatternFlavor::UMin
            | range::SelectPatternFlavor::SMax
            | range::SelectPatternFlavor::UMax
    ) {
        return None;
    }
    match (result.lhs?, result.rhs?) {
        (range::SelectOperand::Value(lhs), range::SelectOperand::Value(rhs)) => Some((lhs, rhs)),
        _ => None,
    }
}

/// `BitCast`
/// ([L6438](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L6438)).
fn bitcast(
    q: &Query,
    op: &Instruction,
    kind: FloatKind,
    interested: FpClassTest,
    known: &mut KnownFpClass,
    depth: u32,
) {
    let src = op.operands[0];
    if !matches!(
        q.type_of(src).map(|ty| q.context().types.get(ty)),
        Some(Type::Int(_))
    ) {
        return;
    }
    let ty = q.type_of(Operand::Value(op.result.expect("a cast has a result")));
    // Match bitcast(umax(bitcast(a), bitcast(b))).
    if let Some((lhs, rhs)) = max_or_min_operands(q, src) {
        let cast_of = |one: Operand| {
            let (_, cast) = q.instruction_of(one)?;
            (cast.opcode == Opcode::Cast(CastOp::BitCast)).then(|| cast.operands[0])
        };
        if let (Some(cast_lhs), Some(cast_rhs)) = (cast_of(lhs), cast_of(rhs))
            && q.type_of(cast_lhs) == ty
            && q.type_of(cast_rhs) == ty
        {
            let known_rhs = compute_known_fp_class(q, cast_rhs, interested, depth + 1);
            if !known_rhs.is_unknown() {
                let known_lhs = compute_known_fp_class(q, cast_lhs, interested, depth + 1);
                *known = known_lhs;
                known.or_assign(&known_rhs);
            }
            return;
        }
    }
    let sem = semantics_of(kind);
    let bits = known::compute_known_bits(q, src, depth + 1);
    if bits.width != crate::apfloat::semantics_size_in_bits(sem) {
        return;
    }
    *known = KnownFpClass::bitcast(sem, &bits);
}

/// `PHI`
/// ([L6363](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L6363)),
/// less the `fma` recurrence (the intrinsics slice).
fn phi(
    q: &Query,
    inst: InstId,
    op: &Instruction,
    interested: FpClassTest,
    known: &mut KnownFpClass,
    depth: u32,
) {
    // Unreachable blocks may have zero-operand PHI nodes.
    if op.operands.is_empty() {
        return;
    }
    // Otherwise take the unions of the known sets of the operands, taking
    // conservative care to avoid excessive recursion.
    let phi_recursion_limit = MAX_ANALYSIS_RECURSION_DEPTH - 2;
    if depth >= phi_recursion_limit {
        return;
    }
    let phi_value = Operand::Value(op.result.expect("a phi has a result"));
    // Skip if every incoming value references to ourself or is undef.
    let incoming: Vec<Operand> = op.operands.chunks(2).map(|pair| pair[0]).filter(|&one| one != phi_value).collect();
    if !incoming.is_empty() && incoming.iter().all(|&one| is_undef(q, one)) {
        return;
    }
    let mut first = true;
    for at in (0..op.operands.len()).step_by(2) {
        let mut cxt = None;
        let value = context::break_self_recursive_phi(q, inst, at, &mut cxt, None);
        // Skip direct self references.
        if value == phi_value {
            continue;
        }
        // Recurse, but cap the recursion to two levels, because we don't want
        // to waste time spinning around in loops. We need at least depth 2 to
        // detect known sign bits.
        let mut inner = *q;
        inner.cond_context = None;
        inner.cxt = cxt;
        let src = compute_known_fp_class(&inner, value, interested, phi_recursion_limit);
        if first {
            *known = src;
            first = false;
        } else {
            known.or_assign(&src);
        }
        if known.get_known_fp_classes() == FC_ALL_FLAGS {
            break;
        }
    }
}

fn is_undef(
    q: &Query,
    v: Operand,
) -> bool {
    matches!(
        v,
        Operand::Constant(id) if q.context().get(id).kind == ConstantKind::Undef
    )
}

/// What `tools/llvmport/fpdump.cpp` prints of every float and double scalar of
/// `module`: the known classes (hex) and sign (`0`, `1`, `?`) asked of
/// `fcAllFlags`, `fcNan` and `fcNegative`.
pub fn dump(
    module: &crate::module::Module,
    dom: bool,
) -> String {
    use super::AssumptionCache;
    use crate::datalayout::DataLayout;
    use crate::domtree::DomTree;
    use crate::module::GlobalKind;

    let layout =
        module.datalayout.as_deref().map(|text| DataLayout::parse(text).expect("a layout")).unwrap_or_default();
    let show = |known: KnownFpClass| {
        let sign = match known.get_sign_bit() {
            Some(true) => '1',
            Some(false) => '0',
            None => '?',
        };
        format!("{:x}/{sign}", known.get_known_fp_classes().0)
    };
    let mut out = String::new();
    for global in &module.globals {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().unwrap_or_default();
        let dt = DomTree::of(function);
        let assumptions = AssumptionCache::new(module, function);
        let conditions = dom.then(|| super::DomConditionCache::new(module, function));
        let first = function.block(function.entry().expect("an entry")).first();
        let mut values: Vec<(Operand, Option<InstId>)> =
            function.parameters().iter().map(|parameter| (Operand::Value(*parameter), first)).collect();
        for (_, inst) in function.walk() {
            if let Some(result) = function.instruction(inst).result {
                values.push((Operand::Value(result), Some(inst)));
            }
        }
        let mut n = 0;
        for (value, cxt) in values {
            let mut query = Query::new(module, function, &layout).with_cxt(cxt);
            query.dt = Some(&dt);
            query.assumptions = Some(&assumptions);
            query.dom_conditions = conditions.as_ref();
            if !matches!(
                float_kind(&query, value),
                Some(FloatKind::Float | FloatKind::Double)
            ) {
                continue;
            }
            out.push_str(&format!(
                "{name} {n} all={} nan={} neg={}\n",
                show(compute_known_fp_class(&query, value, FC_ALL_FLAGS, 0)),
                show(compute_known_fp_class(&query, value, FC_NAN, 0)),
                show(compute_known_fp_class(&query, value, FC_NEGATIVE, 0)),
            ));
            n += 1;
        }
    }
    out
}
