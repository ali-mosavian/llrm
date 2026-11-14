//! `foldSelectInstWithICmp`'s min/max steps: a select that is a min, a max or
//! an absolute value of a compare becomes the intrinsic.
//!
//! Translation of
//! [`canonicalizeSPF`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L1623)
//! at d1106deb71cc, reached from
//! [`foldSelectInstWithICmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2419).
//!
//! MIRROR: `matchDecomposedSelectPattern` of the compare and the two arms;
//! `smin`, `smax`, `umin` and `umax` for the min/max flavors; `abs` (poison at
//! the minimum when the other arm is the `nsw` negation of the first) for the
//! absolute value, with `0 - abs` for its negation; an absolute value is
//! skipped when neither the compare nor its negation has one use.
//!
//! Also the clamp-away-from-an-extreme half of
//! [`foldSelectInstWithICmpConst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2146)
//! (`x == 0 ? 1 : x` is `umax(x, 1)`, and the signed and `max` kin) and
//! [`foldSelectICmpMinMax`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L688)
//! (`x < y ? x : y - 1` is `smin(x, y - 1)`, `y + 1` is `smax`, with the
//! overflow flag of the other sign dropped; `(x <= y) ? 0 : x - y` is
//! `x - smin(y, x)`), each in the pin's order.
//!
//! DEVIATE 1: the other steps of `foldSelectInstWithICmp`
//! (the range half of `foldSelectInstWithICmpConst`, `canonicalizeClampLike`,
//! `tryToReuseConstantFromSelectInComparison`, `foldSelectICmpEq`, the sign-bit
//! swap) are not translated; `foldSelectICmpMinMax` runs here, ahead of them.
//! Exit: translated by their own strips, by 2026-11-24 (owner wasm-3).
//!
//! # Used by
//! `visitSelectInst`.
//!
//! # Example
//! ```text
//! %c = icmp slt i16 %x, 0                 %r = call i16 @llvm.smax.i16(i16 %x, i16 0)
//! %r = select i1 %c, i16 0, i16 %x   ==>
//! ```

use llrm_mir::module::InstId;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate};
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef;
use llrm_mir::valuetracking::nonzero::is_known_non_zero;
use llrm_mir::valuetracking::pattern;
use llrm_mir::valuetracking::range::{
    FastMathFlags, SelectOperand, SelectPatternFlavor, match_decomposed_select_pattern,
};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query, set_flag};
use crate::combine_visit_select::Sel;

/// The operand a pattern result names: a value, or a constant it made.
fn operand_of(
    c: &mut Combiner,
    one: Option<SelectOperand>,
) -> Option<Operand> {
    match one? {
        SelectOperand::Value(operand) => Some(operand),
        SelectOperand::Int { ty, bits } => Some(c.int(ty, bits)),
        _ => None,
    }
}

/// `foldSelectInstWithICmp`'s min/max steps for a select on an `icmp`.
pub(crate) fn select_with_icmp(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    c.context.types.int_bits(sel.ty)?;
    canonicalize_spf(c, inst, sel)
        .or_else(|| clamp_away_from_extreme(c, inst, sel))
        .or_else(|| select_icmp_min_max(c, inst, sel))
}

/// `canonicalizeSPF(Cmp, TrueVal, FalseVal)`.
fn canonicalize_spf(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let q = query(c, Some(inst));
    let (cmp, ..) = pattern::icmp(&q, sel.cond)?;
    let found = match_decomposed_select_pattern(&q, cmp, sel.if_true, sel.if_false, FastMathFlags::default(), false, 0);
    let flavor = found.flavor;
    if flavor == SelectPatternFlavor::None {
        return None;
    }
    let (lhs, rhs) = (operand_of(c, found.lhs)?, operand_of(c, found.rhs)?);
    let base = match flavor {
        SelectPatternFlavor::SMin => "smin",
        SelectPatternFlavor::SMax => "smax",
        SelectPatternFlavor::UMin => "umin",
        SelectPatternFlavor::UMax => "umax",
        SelectPatternFlavor::Abs | SelectPatternFlavor::NAbs => {
            if !has_one_use(c, sel.cond) && !has_one_use(c, rhs) {
                return None;
            }
            let q = query(c, Some(inst));
            let int_min_is_poison = flavor == SelectPatternFlavor::Abs
                && pattern::neg(&q, rhs)
                    .is_some_and(|(_, x, flags)| x == lhs && flags.contains(llrm_mir::opcode::Flags::NSW));
            let flag = c.int(c.context.types.int(1), u128::from(int_min_is_poison));
            let abs = c.create_intrinsic(inst, "abs", sel.ty, &[lhs, flag]);
            if flavor == SelectPatternFlavor::NAbs {
                let zero = c.int(sel.ty, 0);
                return Some(Visit::Replace(c.create_sub(inst, zero, abs)));
            }
            return Some(Visit::Replace(abs));
        }
        _ => return None,
    };
    Some(Visit::Replace(c.create_intrinsic(inst, base, sel.ty, &[lhs, rhs])))
}

/// `(V == UMIN) ? UMIN+1 : V` is `umax(V, UMIN+1)`, `(V == UMAX) ? UMAX-1 : V`
/// is `umin`, and the same at the signed extremes
/// (`foldSelectInstWithICmpConst`, first half).
fn clamp_away_from_extreme(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let width = c.context.types.int_bits(sel.ty)?;
    let q = query(c, Some(inst));
    let (_, _, pred, value, bound) = pattern::icmp(&q, sel.cond)?;
    let (bound, _) = pattern::apint(&q, bound)?;
    if pred.pred != IntPredicate::Eq || value != sel.if_false {
        return None;
    }
    let (replacement, _) = pattern::apint(&q, sel.if_true)?;
    let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
    let signed_min = 1u128 << (width - 1);
    let step = |from: u128, by_one_up: bool| {
        if by_one_up { from.wrapping_add(1) & mask } else { from.wrapping_sub(1) & mask }
    };
    let base = if bound == 0 && replacement == step(bound, true) {
        "umax"
    } else if bound == mask && replacement == step(bound, false) {
        "umin"
    } else if bound == signed_min && replacement == step(bound, true) {
        "smax"
    } else if bound == signed_min - 1 && replacement == step(bound, false) {
        "smin"
    } else {
        return None;
    };
    Some(Visit::Replace(c.create_intrinsic(inst, base, sel.ty, &[value, sel.if_true])))
}

/// `canoncalizeSelectICmpMinMax`: `(X <= Y) ? 0 : (X - Y)` and its three
/// mirror forms are `X - smin(Y, X)`.
fn zero_or_difference(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
    cmp: (IntPredicate, Operand, Operand),
) -> Option<Visit> {
    let q = query(c, Some(inst));
    let (mut pred, mut lhs, mut rhs) = cmp;
    let (mut if_true, mut if_false) = (sel.if_true, sel.if_false);
    if pattern::zero_int(&q, if_false) {
        std::mem::swap(&mut if_true, &mut if_false);
        pred = pred.inverse();
    }
    if !pattern::zero_int(&q, if_true) {
        return None;
    }
    if matches!(pred, IntPredicate::Sgt | IntPredicate::Sge) {
        std::mem::swap(&mut lhs, &mut rhs);
        pred = pred.swapped();
    }
    let (_, diff, a, b) = pattern::binary(&q, if_false, BinaryOp::Sub)?;
    if !matches!(pred, IntPredicate::Slt | IntPredicate::Sle)
        || !diff.flags.contains(Flags::NSW)
        || (a, b) != (lhs, rhs)
        || !is_guaranteed_not_to_be_undef(&q, lhs, 0)
    {
        return None;
    }
    let smin = c.create_intrinsic(inst, "smin", sel.ty, &[rhs, lhs]);
    Some(Visit::Replace(c.create_binop(inst, BinaryOp::Sub, lhs, smin, Flags::NSW)))
}

/// `foldSelectICmpMinMax`: `(X > Y) ? X : (Y - 1)` is `smin(X, Y - 1)` and
/// `(X < Y) ? X : (Y + 1)` is `smax(X, Y + 1)`, valid when the overflow of the
/// compare's sign is poison; the other overflow flag is dropped.
fn select_icmp_min_max(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let q = query(c, Some(inst));
    let (_, _, pred, mut lhs, mut rhs) = pattern::icmp(&q, sel.cond)?;
    let mut pred = pred.pred;
    if let Some(done) = zero_or_difference(c, inst, sel, (pred, lhs, rhs)) {
        return Some(done);
    }
    let q = query(c, Some(inst));
    if rhs == sel.if_true {
        std::mem::swap(&mut lhs, &mut rhs);
        pred = pred.swapped();
    }
    if lhs != sel.if_true {
        return None;
    }
    let (step_inst, step) = q.instruction_of(sel.if_false)?;
    let (_, _, base, amount) = pattern::binary(&q, sel.if_false, BinaryOp::Add)?;
    if base != rhs {
        return None;
    }
    let flags = step.flags;
    let (intrinsic, drop) = match pred {
        IntPredicate::Sgt if pattern::one(&q, amount) && flags.contains(Flags::NSW) => ("smax", &[Flags::NUW][..]),
        IntPredicate::Slt if pattern::all_ones(&q, amount) && flags.contains(Flags::NSW) => ("smin", &[Flags::NUW][..]),
        IntPredicate::Ugt if pattern::one(&q, amount) && flags.contains(Flags::NUW) => ("umax", &[Flags::NSW][..]),
        // `sub nuw %x, 1` became `add %x, -1` and lost its `nuw`.
        IntPredicate::Ult if pattern::all_ones(&q, amount) && is_known_non_zero(&q, rhs, 0) => {
            ("umin", &[Flags::NSW, Flags::NUW][..])
        }
        _ => return None,
    };
    for flag in drop {
        set_flag(c, step_inst, *flag, false);
    }
    Some(Visit::Replace(c.create_intrinsic(inst, intrinsic, sel.ty, &[sel.if_true, sel.if_false])))
}

#[cfg(test)]
#[path = "combine_select_spf_tests.rs"]
mod tests;
