//! `foldSelectToCmp`: a three-way compare written as selects, as `scmp`/`ucmp`.
//!
//! Translation of
//! [`InstCombinerImpl::foldSelectToCmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L4172)
//! at d1106deb71cc.
//!
//! MIRROR: the six forms, in the pin's order, over integers.
//!
//! DEVIATE 1: no vectors (MIR has none).
//!
//! # Example
//! ```text
//! %lt = icmp slt i32 %a, %b
//! %gt = icmp sgt i32 %a, %b
//! %z  = zext i1 %gt to i32                   %r = call i32 @llvm.scmp.i32.i32(i32 %a, i32 %b)
//! %r  = select i1 %lt, i32 -1, i32 %z   ==>
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{CastOp, IntPredicate};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::pattern::{self, Side};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;
use crate::combine_visit_select::Sel;

fn mask(width: u32) -> u128 {
    if width >= 128 { u128::MAX } else { (1u128 << width) - 1 }
}

fn is_gt(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Ugt | IntPredicate::Sgt)
}

fn is_lt(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Ult | IntPredicate::Slt)
}

/// `m_c_ICmp(P, m_Specific(l), m_Specific(r))`: `P` with `l` on the left.
fn c_icmp_sides(
    q: &Query,
    v: Operand,
    l: Side,
    r: Side,
) -> Option<IntPredicate> {
    let (_, _, cmp, a, b) = pattern::icmp(q, v)?;
    let (a, b) = (Side::of(q, a), Side::of(q, b));
    if a == l && b == r {
        Some(cmp.pred)
    } else if a == r && b == l {
        Some(cmp.pred.swapped())
    } else {
        None
    }
}

/// `(signed, lhs, rhs)` of the compare the select spells, and the operand
/// type a constant side is made in.
struct Found {
    signed: bool,
    lhs: Side,
    rhs: Side,
}

fn matched(
    c: &Combiner,
    sel: &Sel,
) -> Option<Found> {
    let q = query(c, None);
    let (_, _, cmp, l, r) = pattern::icmp(&q, sel.cond)?;
    c.context.types.int_bits(c.type_of(l))?;
    let (mut pred, mut lhs, mut rhs) = (cmp.pred, Side::of(&q, l), Side::of(&q, r));
    let (mut tv, mut fv) = (sel.if_true, sel.if_false);
    let constant = |v: Operand| matches!(v, Operand::Constant(_));

    // No -1, 0 or 1 in the true arm: invert the select.
    if !constant(tv) {
        if !constant(fv) {
            return None;
        }
        pred = pred.inverse();
        std::mem::swap(&mut tv, &mut fv);
    }

    // A non-strict compare against a constant becomes the strict one
    // (`getFlippedStrictnessPredicateAndConstant`).
    if pred.true_when_equal() && pred != IntPredicate::Eq {
        let (value, width) = rhs.apint(&q)?;
        let signed = pred.is_signed();
        let increment = matches!(pred, IntPredicate::Ule | IntPredicate::Sle);
        let smax = mask(width) >> 1;
        let (min, max) = if signed { (smax + 1, smax) } else { (0, mask(width)) };
        let edge = if increment { max } else { min };
        if value == edge {
            return None;
        }
        if cmp.same_sign {
            let (omin, omax) = if signed { (0, mask(width)) } else { (smax + 1, smax) };
            if value == if increment { omax } else { omin } {
                return None;
            }
        }
        let next = if increment { value.wrapping_add(1) } else { value.wrapping_sub(1) } & mask(width);
        pred = match pred {
            IntPredicate::Ule => IntPredicate::Ult,
            IntPredicate::Uge => IntPredicate::Ugt,
            IntPredicate::Sle => IntPredicate::Slt,
            _ => IntPredicate::Sgt,
        };
        rhs = Side::Int(next, width);
    }

    // Swap the operands where the arm's constant says the other order.
    if (is_gt(pred) && pattern::all_ones(&q, tv)) || (is_lt(pred) && pattern::one(&q, tv)) {
        pred = pred.swapped();
        std::mem::swap(&mut lhs, &mut rhs);
    }
    let mut signed = pred.is_signed();
    let mut replace = false;

    // (x < y) ? -1 : zext(x != y)    (x < y) ? -1 : zext(x > y)
    // (x > y) ? 1 : sext(x != y)     (x > y) ? 1 : sext(x < y)
    for (strict, arm, ext) in
        [(is_lt(pred), pattern::all_ones(&q, tv), CastOp::ZExt), (is_gt(pred), pattern::one(&q, tv), CastOp::SExt)]
    {
        if strict
            && arm
            && let Some((_, _, inner)) = pattern::cast(&q, fv, ext)
            && let Some(extended) = c_icmp_sides(&q, inner, lhs, rhs)
            && (extended == IntPredicate::Ne || extended.swapped() == pred)
        {
            replace = true;
        }
    }

    // (x == y) ? 0 : (x > y ? 1 : -1)
    if pred == IntPredicate::Eq
        && pattern::zero_int(&q, tv)
        && let Some((_, _, inner_cond, inner_tv, inner_fv)) = pattern::select(&q, fv)
        && let Some((mut inner_tv, _)) = pattern::apint(&q, inner_tv)
        && let Some((mut inner_fv, width)) = pattern::apint(&q, inner_fv)
        && let Some(mut inner_pred) = c_icmp_sides(&q, inner_cond, lhs, rhs)
    {
        if !is_gt(inner_pred) {
            inner_pred = inner_pred.swapped();
            std::mem::swap(&mut lhs, &mut rhs);
        }
        if inner_tv != 1 {
            std::mem::swap(&mut inner_tv, &mut inner_fv);
            std::mem::swap(&mut lhs, &mut rhs);
        }
        if is_gt(inner_pred) && inner_tv == 1 && inner_fv == mask(width) {
            signed = inner_pred.is_signed();
            replace = true;
        }
    }

    // x == C ? 0 : (x > C-1 ? 1 : -1)      x == C ? 0 : (x < C+1 ? -1 : 1)
    if pred == IntPredicate::Eq
        && pattern::zero_int(&q, tv)
        && let Some((value, width)) = rhs.apint(&q)
        && let Some((_, _, inner_cond, inner_tv, inner_fv)) = pattern::select(&q, fv)
        && let Some((_, _, inner, a, b)) = pattern::icmp(&q, inner_cond)
        && Side::of(&q, a) == lhs
        && let Some((inner_tv, _)) = pattern::apint(&q, inner_tv)
        && let Some((inner_fv, _)) = pattern::apint(&q, inner_fv)
    {
        let inner_rhs = Side::of(&q, b);
        let smax = mask(width) >> 1;
        let is = inner.pred.is_signed();
        if is_gt(inner.pred) && inner_tv == 1 && inner_fv == mask(width) {
            signed = is;
            let min = if is { smax + 1 } else { 0 };
            if value != min && inner_rhs == Side::Int(value.wrapping_sub(1) & mask(width), width) {
                replace = true;
            }
        }
        if is_lt(inner.pred) && inner_tv == mask(width) && inner_fv == 1 {
            signed = is;
            let max = if is { smax } else { mask(width) };
            if value != max && inner_rhs == Side::Int(value.wrapping_add(1) & mask(width), width) {
                replace = true;
            }
        }
    }

    replace.then_some(Found { signed, lhs, rhs })
}

pub(crate) fn select_to_cmp(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let found = matched(c, sel)?;
    let cmp = c.function.instruction(c.definition_of(sel.cond)?).operands[0];
    let ty = c.type_of(cmp);
    let side = |c: &mut Combiner, s: Side| match s {
        Side::Value(v) => v,
        Side::Int(bits, _) => c.int(ty, bits),
    };
    let (lhs, rhs) = (side(c, found.lhs), side(c, found.rhs));
    let bits = c.context.types.int_bits(ty)?;
    let result = c.context.types.int_bits(sel.ty)?;
    let name = format!("llvm.{}.i{result}.i{bits}", if found.signed { "scmp" } else { "ucmp" });
    Some(Visit::Replace(c.create_intrinsic_named(inst, &name, sel.ty, &[lhs, rhs])))
}

#[cfg(test)]
#[path = "combine_select_to_cmp_tests.rs"]
mod tests;
