//! InstCombine's `visitFCmpInst`, from
//! [`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L9212)
//! at d1106deb71cc.
//!
//! MIRROR: the order of the steps and each condition. `simplifyFCmpInst` is
//! the generic `simplify_instruction_in` that `visit` runs first.
//!
//! DEVIATE 1: only the opening steps (up to the infinity canonicalisation,
//! L9215-L9325) are translated; the fold-by-operand-kind ones (bitcast, fsub,
//! phi, sitofp, fptrunc, fabs, sqrt, floor/ceil, the min/max-select exit) are
//! not. Exit: the next PR on `combine_fcmp.rs`.
//!
//! DEVIATE 2: only the null value of a vector compare is vector-aware.
//!
//! # Example
//! ```text
//! %c = fcmp olt float %x, 0x7FF0000000000000     %c = fcmp one float %x, 0x7FF0000000000000
//! ```
//!
//! # Used by
//! `combine` (its `visit`, the `FCmp` opcode).

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{FloatPredicate, Opcode};
use llrm_mir::types::Type;
use llrm_mir::valuetracking::fp::compute_known_fp_class;
use llrm_mir::valuetracking::pattern;
use llrm_mir::valuetracking::range::{SelectPatternFlavor, match_select_pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{get_complexity, match_symmetric_pair, query, replace_operand};
use crate::floatfold::float;
use llrm_mir::fpclass::FC_NAN;

/// `visitFCmpInst`.
pub fn visit_fcmp(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    let Opcode::FCmp(_) = c.function.instruction(inst).opcode else { return Visit::None };
    let mut changed = false;
    // Orders the operands from most complex to least complex.
    let [a, b] = c.function.instruction(inst).operands[..] else { return Visit::None };
    if get_complexity(&query(c, Some(inst)), a) < get_complexity(&query(c, Some(inst)), b) {
        let Opcode::FCmp(pred) = c.function.instruction(inst).opcode else { return Visit::None };
        c.function.set_opcode(inst, Opcode::FCmp(pred.swapped()));
        c.function.set_operand(inst, 0, b);
        c.function.set_operand(inst, 1, a);
        changed = true;
    }
    let Opcode::FCmp(pred) = c.function.instruction(inst).opcode else { return Visit::None };
    let [op0, op1] = c.function.instruction(inst).operands[..] else { return Visit::None };
    let ty = c.type_of(op0);
    let zero = zero_of(c, ty);
    // Simplify 'fcmp pred X, X'.
    if op0 == op1 {
        let canonical = match pred {
            FloatPredicate::Uno | FloatPredicate::Ult | FloatPredicate::Ugt | FloatPredicate::Une => {
                Some(FloatPredicate::Uno)
            }
            FloatPredicate::Ord | FloatPredicate::Oeq | FloatPredicate::Oge | FloatPredicate::Ole => {
                Some(FloatPredicate::Ord)
            }
            _ => None,
        };
        if let Some(canonical) = canonical {
            c.function.set_opcode(inst, Opcode::FCmp(canonical));
            c.function.set_operand(inst, 1, zero);
            return Visit::Changed;
        }
    }
    // `FCmpInst::isCommutative`.
    if matches!(
        pred,
        FloatPredicate::Oeq
            | FloatPredicate::One
            | FloatPredicate::Ueq
            | FloatPredicate::Une
            | FloatPredicate::Ord
            | FloatPredicate::Uno
            | FloatPredicate::False
            | FloatPredicate::True
    )
        && let Some((x, y)) = match_symmetric_pair(&query(c, Some(inst)), op0, op1)
    {
        replace_operand(c, inst, 0, x);
        replace_operand(c, inst, 1, y);
        return Visit::Changed;
    }
    // A check for NaN alone, with a non-NaN operand: the operand becomes 0.0.
    if matches!(pred, FloatPredicate::Ord | FloatPredicate::Uno) {
        let q = query(c, Some(inst));
        if !is_pos_zero(c, op0) && compute_known_fp_class(&q, op0, FC_NAN, 0).is_known_never_nan() {
            replace_operand(c, inst, 0, zero);
            return Visit::Changed;
        }
        if !is_pos_zero(c, op1) && compute_known_fp_class(&q, op1, FC_NAN, 0).is_known_never_nan() {
            replace_operand(c, inst, 1, zero);
            return Visit::Changed;
        }
    }
    // fcmp pred (fneg X), (fneg Y) -> fcmp swap(pred) X, Y
    {
        let q = query(c, Some(inst));
        if let (Some(x), Some(y)) = (pattern::fneg(&q, op0), pattern::fneg(&q, op1)) {
            c.function.set_opcode(inst, Opcode::FCmp(pred.swapped()));
            replace_operand(c, inst, 0, x);
            replace_operand(c, inst, 1, y);
            return Visit::Changed;
        }
    }
    // Used only by a select that is a min or a max: refrain from other folds,
    // which keeps the idiom for ScalarEvolution and the selector.
    if used_only_by_min_max_select(c, inst) {
        return if changed { Visit::Changed } else { Visit::None };
    }
    // The sign of 0.0 is ignored by fcmp: fcmp Pred X, -0.0 --> fcmp Pred X, 0.0
    if let Some((_, value)) = float(c.context, op1)
        && value == 0.0
        && value.is_sign_negative()
    {
        replace_operand(c, inst, 1, zero);
        return Visit::Changed;
    }
    // Canonicalize a compare with an infinity.
    if let Some((_, value)) = float(c.context, op1)
        && value.is_infinite()
    {
        let seen = if value.is_sign_negative() { pred.swapped() } else { pred };
        let (new, with_zero) = match seen {
            FloatPredicate::Olt => (FloatPredicate::One, false),
            FloatPredicate::Ole => (FloatPredicate::Ord, true),
            FloatPredicate::Oge => (FloatPredicate::Oeq, false),
            FloatPredicate::Ult => (FloatPredicate::Une, false),
            FloatPredicate::Ugt => (FloatPredicate::Uno, true),
            FloatPredicate::Uge => (FloatPredicate::Ueq, false),
            _ => return if changed { Visit::Changed } else { Visit::None },
        };
        // The pin builds the new compare with `Op1` kept; the infinity's sign
        // was folded into `seen`, so the predicate is not swapped back.
        c.function.set_opcode(inst, Opcode::FCmp(new));
        if with_zero {
            c.function.set_operand(inst, 1, zero);
        }
        return Visit::Changed;
    }
    if changed { Visit::Changed } else { Visit::None }
}

/// `I.hasOneUse()` by a select whose `matchSelectPattern` is not unknown.
///
/// DEVIATE 3: `IsRedundantMinMaxClamp` (`isMinMaxCmpSelectEliminable`) is
/// taken as false: a redundant fmin/fmax clamp keeps its compare. Exit: the
/// next PR on this file.
fn used_only_by_min_max_select(
    c: &Combiner,
    inst: InstId,
) -> bool {
    let Some(result) = c.function.instruction(inst).result else { return false };
    if !c.function.users(result).has_one_use() {
        return false;
    }
    let Some(user) = c.function.users(result).first().map(|one| one.user) else { return false };
    if c.function.instruction(user).opcode != Opcode::Select {
        return false;
    }
    let select = Operand::Value(c.function.instruction(user).result.expect("a select makes a value"));
    match_select_pattern(&query(c, Some(user)), select).flavor != SelectPatternFlavor::None
}

fn zero_of(
    c: &Combiner,
    ty: llrm_mir::types::TypeId,
) -> Operand {
    // `Constant::getNullValue`: `zeroinitializer` for a vector of floats.
    let kind = match *c.context.types.get(ty) {
        Type::Float(kind) => ConstantKind::float_zero(kind),
        Type::Vector { .. } => ConstantKind::Zero,
        _ => unreachable!("an fcmp compares floats"),
    };
    Operand::Constant(c.context.constant(Constant { ty, kind }))
}

fn is_pos_zero(
    c: &Combiner,
    v: Operand,
) -> bool {
    float(c.context, v).is_some_and(|(_, value)| value == 0.0 && !value.is_sign_negative())
}

#[cfg(test)]
#[path = "combine_fcmp_tests.rs"]
mod tests;
