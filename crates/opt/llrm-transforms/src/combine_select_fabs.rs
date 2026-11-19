//! `foldSelectWithFCmpToFabs`: `x < 0.0 ? -x : x` is `fabs(x)` where the user
//! ignores the sign of zero and of a NaN.
//!
//! Translated from
//! [`foldSelectWithFCmpToFabs`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L3321)
//! at d1106deb71cc, with `canIgnoreSignBitOfZero` and `canIgnoreSignBitOfNaN`
//! of `llrm_mir::valuetracking::signbit`. `-0.0` makes it tricky: the fold
//! needs `nsz` or a user that does not read the sign of a zero (`fptosi`,
//! `fcmp`, `fabs`), and `nnan` or a user that does not read the sign of a NaN.
//!
//! # Example
//! ```text
//! %c = fcmp olt x86_fp80 %x, 0.0                  %a = call @llvm.fabs.f80(%x)
//! %n = fneg x86_fp80 %x                  =>       %i = fptosi %a to i32
//! %s = select i1 %c, %n, %x
//! %i = fptosi x86_fp80 %s to i32
//! ```
//! x_ldcmp -O2 at m32 compared and branched for every `|x|` the pin takes
//! with `fabs`.
//!
//! DEVIATE 1: `llvm.fabs` is made with the select's fast-math flags
//! (`CreateFAbs(X, &SI)`), and the result of `-fabs` with its flags, as the
//! pin does; `prof` metadata is not carried (MIR has none on a select).

use llrm_mir::context::ConstantKind;
use llrm_mir::fpclass::FC_NAN;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, FloatPredicate, Opcode};
use llrm_mir::types::{FloatKind, Type};
use llrm_mir::valuetracking::fp::{compute_known_fp_class, is_sign_bit_check};
use llrm_mir::valuetracking::pattern;
use llrm_mir::valuetracking::signbit::{can_ignore_sign_bit_of_nan, can_ignore_sign_bit_of_zero};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;
use crate::combine_visit_select::Sel;

/// The fast-math flags of `flags`.
fn fast_math(flags: Flags) -> Flags {
    flags.intersect(Flags::FAST)
}

/// `m_AnyZeroFP` of a scalar (`only_positive`: `m_PosZeroFP`).
fn zero_fp(
    c: &Combiner,
    v: Operand,
    only_positive: bool,
) -> bool {
    let Operand::Constant(id) = v else { return false };
    let constant = c.context.get(id);
    let (ConstantKind::Float(bits), Type::Float(kind)) = (&constant.kind, c.context.types.get(constant.ty)) else {
        return false;
    };
    let sign = if *kind == FloatKind::Float { 1_u64 << 31 } else { 1_u64 << 63 };
    if only_positive { *bits == 0 } else { bits & !sign == 0 }
}

/// `llvm.fabs.<f32|f64|f80>`'s suffix of the float type of `v`.
fn fabs_name(
    c: &Combiner,
    v: Operand,
) -> Option<&'static str> {
    match c.context.types.get(c.type_of(v)) {
        Type::Float(FloatKind::Float) => Some("llvm.fabs.f32"),
        Type::Float(FloatKind::Double) => Some("llvm.fabs.f64"),
        Type::Float(FloatKind::X86Fp80) => Some("llvm.fabs.f80"),
        _ => None,
    }
}

/// `Builder.CreateFAbs(X, &SI)`: before `at`, with the fast-math flags of
/// `select`.
fn create_fabs(
    c: &mut Combiner,
    at: InstId,
    select: InstId,
    x: Operand,
) -> Option<Operand> {
    let name = fabs_name(c, x)?;
    let ty = c.type_of(x);
    let call = c.create_intrinsic_named(at, name, ty, &[x]);
    let Operand::Value(value) = call else { return None };
    let (inst, _) = query(c, None).instruction_of(Operand::Value(value))?;
    let flags = fast_math(c.function.instruction(select).flags);
    c.function.set_flags(inst, flags);
    Some(call)
}

/// `foldSelectWithFCmpToFabs` (:3321).
pub(crate) fn select_fcmp_to_fabs(
    c: &mut Combiner,
    select: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let (cond_inst, cond) = {
        let (inst, instruction) = query(c, None).instruction_of(sel.cond)?;
        (inst, instruction.clone())
    };
    let mut changed_fmf = false;
    if let Opcode::FCmp(pred) = cond.opcode {
        for swap in [false, true] {
            let (mut true_val, mut x) = (sel.if_true, sel.if_false);
            if swap {
                std::mem::swap(&mut true_val, &mut x);
            }
            let [lhs, rhs] = cond.operands[..] else { continue };
            if lhs != x || !zero_fp(c, rhs, false) {
                continue;
            }
            let select_flags = c.function.instruction(select).flags;
            let only_use = match sel_result(c, select) {
                Some(value) => c.function.users(value).only().copied(),
                None => None,
            };
            let ignores_nan = |c: &Combiner, flags: Flags| {
                flags.contains(Flags::NNAN)
                    || only_use.is_some_and(|used| can_ignore_sign_bit_of_nan(&query(c, None), used))
            };
            let ignores_zero = |c: &Combiner, flags: Flags| {
                flags.contains(Flags::NSZ)
                    || only_use.is_some_and(|used| can_ignore_sign_bit_of_zero(&query(c, None), used))
            };

            // fold (X <= +/-0.0) ? (0.0 - X) : X to fabs(X), when 'Swap' is
            // false fold (X >  +/-0.0) ? X : (0.0 - X) to fabs(X),
            // when 'Swap' is true
            if let Some((_, fsub)) = query(c, None).instruction_of(true_val)
                && fsub.opcode == Opcode::Binary(BinaryOp::FSub)
                && zero_fp(c, fsub.operands[0], true)
                && fsub.operands[1] == x
                && (cond.flags.contains(Flags::NNAN)
                    || ignores_nan(c, select_flags)
                    || compute_known_fp_class(&query(c, Some(cond_inst)), x, FC_NAN, 0).is_known_never_nan())
            {
                let ordered_le = matches!(pred, FloatPredicate::Ole | FloatPredicate::Ule);
                let ordered_gt = matches!(pred, FloatPredicate::Ogt | FloatPredicate::Ugt);
                if (!swap && ordered_le) || (swap && ordered_gt) {
                    let fabs = create_fabs(c, select, select, x)?;
                    return Some(Visit::Replace(fabs));
                }
            }

            // The pin gives up on the whole fold here, not on this arm order.
            pattern::fneg(&query(c, None), true_val).filter(|&source| source == x)?;
            let flip = query(c, None).instruction_of(true_val)?.1.clone();

            // Forward-propagate nnan and ninf from the fcmp to the select.
            let mut flags = select_flags;
            if cond.flags.contains(Flags::NNAN) && !flags.contains(Flags::NNAN) {
                flags = flags.union(Flags::NNAN);
                changed_fmf = true;
            }
            if cond.flags.contains(Flags::NINF) && !flags.contains(Flags::NINF) {
                flags = flags.union(Flags::NINF);
                changed_fmf = true;
            }
            // The nnan flag can be propagated iff fneg is selected when X is
            // NaN.
            if !flags.contains(Flags::NNAN)
                && flip.flags.contains(Flags::NNAN)
                && (if swap { pred.is_ordered() } else { pred.is_unordered() })
            {
                flags = flags.union(Flags::NNAN);
                changed_fmf = true;
            }
            if flags != select_flags {
                c.function.set_flags(select, flags);
            }

            // With nsz and nnan (or users that read neither sign):
            // fold (X < +/-0.0) ? -X : X or (X <= +/-0.0) ? -X : X to fabs(X)
            // fold (X > +/-0.0) ? -X : X or (X >= +/-0.0) ? -X : X to -fabs(x)
            // and mirrored for 'Swap'.
            if !ignores_zero(c, flags) || !ignores_nan(c, flags) {
                return changed_fmf.then_some(Visit::Changed);
            }
            let pred = if swap { pred.swapped() } else { pred };
            let lt_or_le =
                matches!(
                    pred,
                    FloatPredicate::Olt | FloatPredicate::Ole | FloatPredicate::Ult | FloatPredicate::Ule
                );
            let gt_or_ge =
                matches!(
                    pred,
                    FloatPredicate::Ogt | FloatPredicate::Oge | FloatPredicate::Ugt | FloatPredicate::Uge
                );
            if lt_or_le {
                let fabs = create_fabs(c, select, select, x)?;
                return Some(Visit::Replace(fabs));
            }
            if gt_or_ge {
                let fabs = create_fabs(c, select, select, x)?;
                let ty = c.type_of(fabs);
                let flags = fast_math(c.function.instruction(select).flags);
                return Some(c.replace_with_new(select, Opcode::FNeg, ty, vec![fabs], flags));
            }
        }
    }

    // Match select with (icmp slt (bitcast X to int), 0)
    //                or (icmp sgt (bitcast X to int), -1)
    if let Opcode::ICmp(pred) = cond.opcode {
        for swap in [false, true] {
            let (mut true_val, mut x) = (sel.if_true, sel.if_false);
            if swap {
                std::mem::swap(&mut true_val, &mut x);
            }
            let [lhs, rhs] = cond.operands[..] else { continue };
            // m_ICmp(Pred, m_ElementWiseBitCast(m_Specific(X)), m_APInt(C))
            let q = query(c, None);
            let Some((_, bitcast)) = q.instruction_of(lhs) else { continue };
            if bitcast.opcode != Opcode::Cast(CastOp::BitCast) || bitcast.operands[0] != x {
                continue;
            }
            let Some((bits, width)) = q.int_constant(rhs) else { continue };
            let Some(true_if_signed) = is_sign_bit_check(pred, bits, width) else { continue };
            if pattern::fneg(&q, true_val) != Some(x) {
                return None;
            }
            let cond_one_use = c.has_one_use(sel.cond);
            let true_one_use = c.has_one_use(true_val);
            if swap == true_if_signed && !cond_one_use && !true_one_use {
                return None;
            }
            // Fold (IsNeg ? -X : X) or (!IsNeg ? X : -X) to fabs(X)
            // Fold (IsNeg ? X : -X) or (!IsNeg ? -X : X) to -fabs(X)
            let fabs = create_fabs(c, select, select, x)?;
            if swap != true_if_signed {
                return Some(Visit::Replace(fabs));
            }
            let ty = c.type_of(fabs);
            let flags = fast_math(c.function.instruction(select).flags);
            return Some(c.replace_with_new(select, Opcode::FNeg, ty, vec![fabs], flags));
        }
    }

    changed_fmf.then_some(Visit::Changed)
}

/// The value `select` makes.
fn sel_result(
    c: &Combiner,
    select: InstId,
) -> Option<llrm_mir::module::ValueId> {
    c.function.instruction(select).result
}

#[cfg(test)]
#[path = "combine_select_fabs_tests.rs"]
mod tests;
