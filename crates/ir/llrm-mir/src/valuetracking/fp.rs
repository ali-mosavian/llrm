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
//! - DEVIATE 2: `computeKnownFPClassFromContext` (`llvm.assume` and dominating
//!   conditions) and `adjustKnownFPClassForSelectArm` are not translated, so a
//!   `select` arm is not narrowed by its condition. Exit: the context slice.
//! - DEVIATE 3: a function's `denormal-fp-math` is not read; every function is
//!   IEEE (`DenormalMode::getIEEE`), as `getDenormalMode` answers for a
//!   function with no attribute.
//! - DEVIATE 4: no `nofpclass` on an argument, a call result or a load's
//!   `!nofpclass`; no `amdgcn` intrinsic. An `x86_fp80` constant is held as the
//!   nearest double, so it is unknown. The intrinsics other than `fabs`,
//!   `copysign` and `floor` (read by `isAbsoluteValueULEOne`) are the next
//!   slice, and so are `bitcast` and the `fma` recurrence of a phi.
//!
//! # Example
//! `uitofp i32 %x to float` is never a NaN, a negative or a subnormal.
//!
//! # Used by
//! Not yet: the folds that read it follow.

use super::pattern::{self, IntrinsicId};
use super::query::Query;
use super::{MAX_ANALYSIS_RECURSION_DEPTH, context, guarantee, known};
use crate::apfloat::{ApFloat, IEEE_DOUBLE, IEEE_SINGLE, Sem, semantics_of};
use crate::context::ConstantKind;
use crate::fpclass::{
    DenormalMode, FC_ALL_FLAGS, FC_INF, FC_NAN, FC_NEG_NORMAL, FC_NEG_SUBNORMAL, FC_NEG_ZERO, FC_NEGATIVE, FC_NONE,
    FC_NORMAL, FC_POS_NORMAL, FC_POS_SUBNORMAL, FC_POS_ZERO, FC_POSITIVE, FC_SUBNORMAL, FC_ZERO, FpClassTest,
};
use crate::knownfpclass::KnownFpClass;
use crate::module::{InstId, Instruction, Operand};
use crate::opcode::{BinaryOp, CastOp, Flags, Opcode};
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
    let Some((inst, instruction)) = q.instruction_of(v) else { return };

    // The classes the flags rule out.
    let mut not_from_flags = FC_NONE;
    if instruction.flags.contains(Flags::NNAN) {
        not_from_flags |= FC_NAN;
    }
    if instruction.flags.contains(Flags::NINF) {
        not_from_flags |= FC_INF;
    }
    // We no longer need to find out about these bits from inputs.
    interested &= !not_from_flags;

    // All recursive calls that increase depth must come after this.
    if depth < MAX_ANALYSIS_RECURSION_DEPTH {
        of_operator(q, inst, instruction, kind, interested, known, depth);
    }
    known.known_not(not_from_flags);
}

#[allow(clippy::too_many_arguments)]
fn of_operator(
    q: &Query,
    inst: InstId,
    op: &Instruction,
    kind: FloatKind,
    interested: FpClassTest,
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
        // L5352: DEVIATE 2, no arm adjustment.
        Opcode::Select => {
            *known = again(op.operands[1], interested).intersect_with(&again(op.operands[2], interested));
        }
        Opcode::Call(_) => of_call(q, inst, op, interested, known, depth),
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub) => fadd_fsub(q, op, interested, known, depth),
        Opcode::Binary(BinaryOp::FMul) => fmul(q, op, known, depth),
        Opcode::Binary(BinaryOp::FDiv) => fdiv(q, op, interested, known, depth),
        Opcode::Binary(BinaryOp::FRem) => frem(q, op, interested, known, depth),
        Opcode::Cast(CastOp::FPExt) => {
            let src = again(op.operands[0], interested);
            let (Some(dst_sem), Some(src_sem)) = (Some(semantics_of(kind)), sem_of(q, op.operands[0])) else { return };
            *known = KnownFpClass::fpext(&src, dst_sem, src_sem);
        }
        // L6176, `computeKnownFPClassForFPTrunc`.
        Opcode::Cast(CastOp::FPTrunc) => {
            if (interested & (KnownFpClass::ORDERED_LESS_THAN_ZERO_MASK | FC_NAN)) == FC_NONE {
                return;
            }
            *known = KnownFpClass::fptrunc(&again(op.operands[0], interested));
        }
        Opcode::Cast(cast @ (CastOp::SIToFP | CastOp::UIToFP)) => {
            int_to_fp(q, op, *cast == CastOp::SIToFP, kind, interested, known, depth);
        }
        Opcode::Phi => phi(q, inst, op, interested, known, depth),
        _ => {}
    }
}

/// `fabs` and `copysign`, the intrinsics `isAbsoluteValueULEOne`'s neighbours
/// need first.
fn of_call(
    q: &Query,
    inst: InstId,
    op: &Instruction,
    interested: FpClassTest,
    known: &mut KnownFpClass,
    depth: u32,
) {
    let again = |operand: Operand, classes: FpClassTest| compute_known_fp_class(q, operand, classes, depth + 1);
    let arguments = pattern::call_arguments(op);
    match pattern::call_intrinsic(q, inst) {
        // L5393
        Some(IntrinsicId::Fabs) => {
            if (interested & (FC_NAN | FC_POSITIVE)) != FC_NONE {
                // If we only care about the sign bit we don't need to inspect
                // the operand.
                *known = again(arguments[0], interested);
            }
            known.fabs();
        }
        // L5403
        Some(IntrinsicId::Copysign) => {
            *known = again(arguments[0], interested);
            let sign = again(arguments[1], interested);
            known.copysign(&sign);
        }
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
    let mut known_lhs = KnownFpClass::default();
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
pub fn dump(module: &crate::module::Module) -> String {
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
        let name = global.name.clone().unwrap_or_default();
        let dt = DomTree::of(function);
        let assumptions = AssumptionCache::new(module, function);
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
