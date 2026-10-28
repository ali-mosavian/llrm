//! Range folds of `foldAndOrOfICmps`: two compares of one value, each a
//! range of it, are one compare.
//!
//! Translation of
//! [`simplifyRangeCheck`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L726)
//! and
//! [`foldAndOrOfICmpsUsingRanges`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L1341)
//! at d1106deb71cc.
//!
//! # How it works
//! `foldAndOrOfICmpsUsingRanges` reads each compare of a constant as the
//! `ConstantRange` of values it accepts (of `X` for `(X + C1) pred C`, or of
//! `X` for `(X & -2^k) ==/!= C`). Where both are ranges of the same `X`, an
//! `or` accepts their union and an `and` the inverse of the union of the
//! inverses. A union the range type holds exactly is one compare
//! (`getEquivalentICmp`: `X u< n`, or `(X + offset) u< n`); two equal-size
//! ranges that differ in one bit of both ends are one compare of `X` with
//! that bit cleared. `simplifyRangeCheck` is the signed case with a lower bound
//! of 0 and a variable upper bound known not negative: one unsigned compare.
//!
//! # Example
//! A lower-case test of a `char` (`c >= 'a' && c <= 'z'`):
//!
//! ```text
//! %a = icmp sgt i8 %c, 96                     %s = add i8 %c, -97
//! %b = icmp slt i8 %c, 123          ==>       %r = icmp ult i8 %s, 26
//! %r = and i1 %a, %b
//! ```
//!
//! MIRROR: the C++ order of the cases and its one-use conditions.
//!
//! DEVIATE 1: scalars only. The pin matches a splat vector constant as well
//! (`m_APInt`). Exit: the vector `ConstantRange`.
//!
//! # Used by
//! `combine_icmp_logic::fold_boolean_and_or`.

use llrm_mir::constantrange::{ConstantRange, Range};
use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate};
use llrm_mir::valuetracking::{Query, known, pattern};

use crate::combine::Combiner;
use crate::combine_shared::{has_one_use, query};

fn is_equality(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
}

/// Whether `v` is a flags result, which only a comparison may read: the `and`
/// or `add` a fold gives it would leave isel nothing to read the flags with
/// (`result_read_only_by_comparison`).
fn reads_only_by_comparison(
    c: &Combiner,
    v: Operand,
) -> bool {
    matches!(
        v,
        Operand::Value(value) if llrm_mir::facts::result_read_only_by_comparison( c.context, c.env.globals, c.function, value )
    )
}

/// `m_SExtOrSelf(m_Specific(input))`.
fn is_sext_or_self(
    q: &Query,
    v: Operand,
    input: Operand,
) -> bool {
    v == input || pattern::cast(q, v, CastOp::SExt).is_some_and(|(_, _, x)| x == input)
}

/// `APInt::isNegatedPowerOf2`: negative, and its ones are one run to the top.
fn is_negated_power2(
    bits: u128,
    width: u32,
) -> bool {
    let ones = (bits << (128 - width)).leading_ones();
    bits >> (width - 1) == 1 && ones + bits.trailing_zeros().min(width) == width
}

/// `simplifyRangeCheck`: `(icmp sge x, 0) & (icmp slt x, n)` is
/// `icmp ult x, n` (`n` not negative); with `inverted`, the `or` of the
/// inverted ranges is `icmp ugt`.
pub(crate) fn simplify_range_check(
    c: &mut Combiner,
    inst: InstId,
    mut pred_l: IntPredicate,
    lhs0: Operand,
    lhs1: Operand,
    mut pred_r: IntPredicate,
    rhs0: Operand,
    rhs1: Operand,
    inverted: bool,
) -> Option<Operand> {
    let (input, range_end, new_pred) = {
        let q = query(c, Some(inst));
        // Check the lower range comparison, e.g. x >= 0.
        let (start, width) = q.int_constant(lhs1)?;
        if inverted {
            pred_l = pred_l.inverse();
            pred_r = pred_r.inverse();
        }
        // Accept x > -1 or x >= 0 (after potentially inverting the predicate).
        if !((pred_l == IntPredicate::Sgt && start == mask(width)) || (pred_l == IntPredicate::Sge && start == 0)) {
            return None;
        }
        let (input, range_end) = if is_sext_or_self(&q, rhs0, lhs0) {
            (rhs0, rhs1)
        } else if is_sext_or_self(&q, rhs1, lhs0) {
            pred_r = pred_r.swapped();
            (rhs1, rhs0)
        } else {
            return None;
        };
        let new_pred = match pred_r {
            IntPredicate::Slt => IntPredicate::Ult,
            IntPredicate::Sle => IntPredicate::Ule,
            _ => return None,
        };
        // Only valid if the upper range is not negative.
        if !known::compute_known_bits(&q, range_end, 0).is_non_negative() {
            return None;
        }
        (input, range_end, new_pred)
    };
    let new_pred = if inverted { new_pred.inverse() } else { new_pred };
    Some(c.create_icmp(inst, new_pred, input, range_end))
}

/// `(V, CR)` for a compare of `lhs` with the constant `rhs` that is a range
/// check of `V` (`MatchExactRangeCheck`).
fn match_exact_range_check(
    c: &Combiner,
    inst: InstId,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<(Operand, ConstantRange)> {
    let q = query(c, Some(inst));
    let (value, width) = q.int_constant(rhs)?;
    // (x & NegPow2) ==/!= C
    if is_equality(pred)
        && has_one_use(c, lhs)
        && let Some((_, _, x, m)) = pattern::binary(&q, lhs, BinaryOp::And)
        && let Some((neg_pow2, _)) = q.int_constant(m)
        && is_negated_power2(neg_pow2, width)
        && value.trailing_zeros().min(width) >= neg_pow2.trailing_zeros()
    {
        let range = Range::new(width, value, value.wrapping_sub(neg_pow2) & mask(width));
        return Some((x, if pred == IntPredicate::Ne { range.inverse() } else { range }));
    }
    let range = Range::make_exact_icmp_region(pred, width, value);
    // (add X, C1) pred C
    if let Some((x, offset)) = pattern::add_like(&q, lhs, Flags::default())
        && let Some((offset, _)) = q.int_constant(offset)
    {
        return Some((x, range.subtract(offset)));
    }
    Some((lhs, range))
}

/// `foldAndOrOfICmpsUsingRanges`.
pub(crate) fn fold_and_or_of_icmps_using_ranges(
    c: &mut Combiner,
    inst: InstId,
    pred_l: IntPredicate,
    (lhs0, lhs1): (Operand, Operand),
    pred_r: IntPredicate,
    (rhs0, rhs1): (Operand, Operand),
    (lhs_one_use, rhs_one_use): (bool, bool),
    is_and: bool,
) -> Option<Operand> {
    let (v1, mut cr1) = match_exact_range_check(c, inst, pred_l, lhs0, lhs1)?;
    let (v2, mut cr2) = match_exact_range_check(c, inst, pred_r, rhs0, rhs1)?;
    if v1 != v2 {
        return None;
    }
    // For 'and', De Morgan's laws simplify the implementation.
    if is_and {
        cr1 = cr1.inverse();
        cr2 = cr2.inverse();
    }
    let ty = c.type_of(v1);
    let width = cr1.width;
    let mut new_v = v1;
    let range = match cr1.exact_union_with(&cr2) {
        Some(union) => union,
        None => {
            if !lhs_one_use || !rhs_one_use || cr1.is_wrapped_set() || cr2.is_wrapped_set() {
                return None;
            }
            // Equal-size ranges that differ by one bit: a mask maps one onto
            // the other.
            let all = mask(width);
            let lower_diff = cr1.lower ^ cr2.lower;
            let upper_diff = cr1.upper.wrapping_sub(1) & all ^ cr2.upper.wrapping_sub(1) & all;
            let size = |r: &ConstantRange| r.upper.wrapping_sub(r.lower) & all;
            if !lower_diff.is_power_of_two() || lower_diff != upper_diff || size(&cr1) != size(&cr2) {
                return None;
            }
            let lower = if cr1.lower < cr2.lower { cr1 } else { cr2 };
            if reads_only_by_comparison(c, v1) {
                return None;
            }
            let keep = c.int(ty, !lower_diff & all);
            new_v = c.create_binop(inst, BinaryOp::And, new_v, keep, Flags::default());
            lower
        }
    };
    let range = if is_and { range.inverse() } else { range };
    let (pred, new_c, offset) = range.get_equivalent_icmp_offset();
    if offset != 0 {
        if reads_only_by_comparison(c, v1) {
            return None;
        }
        let offset = c.int(ty, offset);
        new_v = c.create_add(inst, new_v, offset);
    }
    let new_c = c.int(ty, new_c);
    Some(c.create_icmp(inst, pred, new_v, new_c))
}
