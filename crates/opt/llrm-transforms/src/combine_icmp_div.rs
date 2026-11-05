//! `foldICmpDivConstant`: `icmp pred ([us]div X, C2), C` is a range test of
//! `X`.
//!
//! Translation of
//! [`foldICmpDivConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L2857)
//! and
//! [`insertRangeTest`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L61).
//!
//! MIRROR: the order of the folds, each condition and the overflow bookkeeping.
//!
//! DEVIATE 1: values are `u128` masked to the type's width (`APInt`), so the
//! overflow of the `addWithOverflow` and `subWithOverflow` helpers is
//! computed on sign-extended `i128`s.
//!
//! # How it works
//! `X / C2 == C` solves to `X` in `[C * C2, C * C2 + C2)`; an overflow of
//! either bound turns the test into a one-sided compare or a constant.
//!
//! # Used by
//! `combine_icmp_const::fold_icmp_inst_with_constant`.
//!
//! # Example
//! ```text
//! %x = sdiv i32 %v, 1024
//! %r = icmp slt i32 %x, 0         ; %r = icmp slt i32 %v, -1023
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::known;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};

/// The signed value of `v` of `width` bits.
fn signed(
    v: u128,
    width: u32,
) -> i128 {
    let shift = 128 - width;
    ((v << shift) as i128) >> shift
}

fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

/// `addWithOverflow`: the sum and whether it overflowed `width` bits.
fn add_with_overflow(
    a: u128,
    b: u128,
    width: u32,
    signed_op: bool,
) -> (u128, bool) {
    let m = mask(width);
    let sum = a.wrapping_add(b) & m;
    let overflow = if signed_op {
        match signed(a, width).checked_add(signed(b, width)) {
            Some(exact) => signed(sum, width) != exact,
            None => true,
        }
    } else {
        a.checked_add(b).is_none_or(|exact| exact > m)
    };
    (sum, overflow)
}

/// `subWithOverflow`: the difference and whether it overflowed `width` bits.
fn sub_with_overflow(
    a: u128,
    b: u128,
    width: u32,
    signed_op: bool,
) -> (u128, bool) {
    let m = mask(width);
    let diff = a.wrapping_sub(b) & m;
    let overflow = if signed_op {
        match signed(a, width).checked_sub(signed(b, width)) {
            Some(exact) => signed(diff, width) != exact,
            None => true,
        }
    } else {
        a < b
    };
    (diff, overflow)
}

/// `insertRangeTest(V, Lo, Hi, isSigned, Inside)`: whether `V` is in
/// `[Lo, Hi)` (or outside it).
fn insert_range_test(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
    (lo, hi): (u128, u128),
    width: u32,
    (signed_range, inside): (bool, bool),
) -> Operand {
    let ty = c.type_of(v);
    // V >= Min && V <  Hi --> V <  Hi
    // V <  Min || V >= Hi --> V >= Hi
    let mut pred = if inside { IntPredicate::Ult } else { IntPredicate::Uge };
    let is_min = if signed_range { lo == 1u128 << (width - 1) } else { lo == 0 };
    if is_min {
        if signed_range {
            pred = match pred {
                IntPredicate::Ult => IntPredicate::Slt,
                _ => IntPredicate::Sge,
            };
        }
        let hi = c.int(ty, hi);
        return c.create_icmp(at, pred, v, hi);
    }
    // V >= Lo && V <  Hi --> V - Lo u<  Hi - Lo
    // V <  Lo || V >= Hi --> V - Lo u>= Hi - Lo
    let lo_const = c.int(ty, lo);
    let minus_lo = c.create_binop(at, BinaryOp::Sub, v, lo_const, Flags::default());
    let span = c.int(ty, hi.wrapping_sub(lo) & mask(width));
    c.create_icmp(at, pred, minus_lo, span)
}

/// `foldICmpDivConstant(Cmp, Div, C)`.
pub fn fold_icmp_div_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    div: Operand,
    (op, flags): (BinaryOp, Flags),
    (x, y): (Operand, Operand),
    value: u128,
    width: u32,
) -> Option<Visit> {
    let ty = c.type_of(x);
    let m = mask(width);
    let equality = matches!(pred, IntPredicate::Eq | IntPredicate::Ne);
    let mut div_signed = op == BinaryOp::SDiv;
    let i1 = c.context.types.int(1);

    // (X u/ Y) == C --> (X == C) && (Y == 1), and the signed SMIN case.
    if equality
        && has_one_use(c, div)
        && value >> (width - 1) & 1 == 1
        && (!div_signed || value == 1u128 << (width - 1))
    {
        let x_big_const = c.int(ty, value);
        let x_big = c.create_icmp(inst, pred, x, x_big_const);
        let one = c.int(ty, 1);
        let y_one = c.create_icmp(inst, pred, y, one);
        let logic = if pred == IntPredicate::Eq { BinaryOp::And } else { BinaryOp::Or };
        return Some(c.replace_with_new(inst, Opcode::Binary(logic), i1, vec![x_big, y_one], Flags::default()));
    }

    // Fold: icmp pred ([us]div X, C2), C -> range test.
    let (c2, _) = query(c, None).int_constant(y)?;
    let c2_signed = signed(c2, width);

    // The divide and the compare must agree in signedness, unless a positive
    // divisor of a non-negative dividend makes the sdiv a udiv.
    if !equality && div_signed != is_signed(pred) {
        if !div_signed || c2_signed <= 0 {
            return None;
        }
        let q = query(c, Some(inst));
        if !known::compute_known_bits(&q, x, 0).is_non_negative() {
            return None;
        }
        div_signed = false;
    }

    // The ProdOV computation fails on divide by 0 and divide by -1.
    if c2 == 0 || c2 == 1 || (div_signed && c2 == m) {
        return None;
    }

    let prod = value.wrapping_mul(c2) & m;
    let prod_ov = if div_signed {
        (signed(prod, width).wrapping_div(c2_signed) as u128 & m) != value
    } else {
        prod / c2 != value
    };

    // If the division is known to be exact, the covered range size is unit.
    let mut range_size = if flags.contains(Flags::EXACT) { 1 } else { c2 };

    let mut pred = pred;
    let (mut lo_ov, mut hi_ov) = (0i32, 0i32);
    let (mut lo_bound, mut hi_bound) = (0u128, 0u128);
    let neg = |v: u128| v.wrapping_neg() & m;
    let value_signed = signed(value, width);

    if !div_signed {
        // e.g. X/5 op 3  --> [15, 20)
        lo_bound = prod;
        lo_ov = i32::from(prod_ov);
        hi_ov = lo_ov;
        if hi_ov == 0 {
            let (sum, overflow) = add_with_overflow(lo_bound, range_size, width, false);
            hi_bound = sum;
            hi_ov = i32::from(overflow);
        }
    } else if c2_signed > 0 {
        if value == 0 {
            // Can't overflow.  e.g.  X/2 op 0 --> [-1, 2)
            lo_bound = neg(range_size.wrapping_sub(1) & m);
            hi_bound = range_size;
        } else if value_signed > 0 {
            // e.g.   X/5 op 3 --> [15, 20)
            lo_bound = prod;
            lo_ov = i32::from(prod_ov);
            hi_ov = lo_ov;
            if hi_ov == 0 {
                let (sum, overflow) = add_with_overflow(prod, range_size, width, true);
                hi_bound = sum;
                hi_ov = i32::from(overflow);
            }
        } else {
            // e.g. X/5 op -3  --> [-15-4, -15+1) --> [-19, -14)
            hi_bound = prod.wrapping_add(1) & m;
            lo_ov = if prod_ov { -1 } else { 0 };
            hi_ov = lo_ov;
            if lo_ov == 0 {
                let (sum, overflow) = add_with_overflow(hi_bound, neg(range_size), width, true);
                lo_bound = sum;
                lo_ov = if overflow { -1 } else { 0 };
            }
        }
    } else {
        // Divisor is < 0.
        if flags.contains(Flags::EXACT) {
            range_size = neg(range_size);
        }
        if value == 0 {
            // e.g. X/-5 op 0  --> [-4, 5)
            lo_bound = range_size.wrapping_add(1) & m;
            hi_bound = neg(range_size);
            if hi_bound == c2 {
                // -INTMIN = INTMIN: [INTMIN+1, overflow)
                hi_ov = 1;
                hi_bound = 0;
            }
        } else if value_signed > 0 {
            // e.g. X/-5 op 3  --> [-19, -14)
            hi_bound = prod.wrapping_add(1) & m;
            lo_ov = if prod_ov { -1 } else { 0 };
            hi_ov = lo_ov;
            if lo_ov == 0 {
                let (sum, overflow) = add_with_overflow(hi_bound, range_size, width, true);
                lo_bound = sum;
                lo_ov = if overflow { -1 } else { 0 };
            }
        } else {
            // e.g. X/-5 op -3  --> [15, 20)
            lo_bound = prod;
            lo_ov = i32::from(prod_ov);
            hi_ov = lo_ov;
            if hi_ov == 0 {
                let (diff, overflow) = sub_with_overflow(prod, range_size, width, true);
                hi_bound = diff;
                hi_ov = i32::from(overflow);
            }
        }
        // Dividing by a negative swaps the condition.  LT <-> GT
        pred = pred.swapped();
    }

    let (lo_c, hi_c) = (c.int(ty, lo_bound), c.int(ty, hi_bound));
    let (ge, lt) =
        if div_signed { (IntPredicate::Sge, IntPredicate::Slt) } else { (IntPredicate::Uge, IntPredicate::Ult) };
    let constant = |c: &mut Combiner, truth: bool| Some(Visit::Replace(c.int(i1, u128::from(truth))));
    match pred {
        IntPredicate::Eq => {
            if lo_ov != 0 && hi_ov != 0 {
                return constant(c, false);
            }
            if hi_ov != 0 {
                return Some(new_icmp(c, inst, ge, x, lo_c));
            }
            if lo_ov != 0 {
                return Some(new_icmp(c, inst, lt, x, hi_c));
            }
            let test = insert_range_test(c, inst, x, (lo_bound, hi_bound), width, (div_signed, true));
            Some(Visit::Replace(test))
        }
        IntPredicate::Ne => {
            if lo_ov != 0 && hi_ov != 0 {
                return constant(c, true);
            }
            if hi_ov != 0 {
                return Some(new_icmp(c, inst, lt, x, lo_c));
            }
            if lo_ov != 0 {
                return Some(new_icmp(c, inst, ge, x, hi_c));
            }
            let test = insert_range_test(c, inst, x, (lo_bound, hi_bound), width, (div_signed, false));
            Some(Visit::Replace(test))
        }
        IntPredicate::Ult | IntPredicate::Slt => {
            if lo_ov == 1 {
                // Low bound is greater than input range.
                return constant(c, true);
            }
            if lo_ov == -1 {
                // Low bound is less than input range.
                return constant(c, false);
            }
            Some(new_icmp(c, inst, pred, x, lo_c))
        }
        IntPredicate::Ugt | IntPredicate::Sgt => {
            if hi_ov == 1 {
                // High bound greater than input range.
                return constant(c, false);
            }
            if hi_ov == -1 {
                // High bound less than input range.
                return constant(c, true);
            }
            let ge = if pred == IntPredicate::Ugt { IntPredicate::Uge } else { IntPredicate::Sge };
            Some(new_icmp(c, inst, ge, x, hi_c))
        }
        _ => None,
    }
}

/// `new ICmpInst(pred, a, b)` in place of `inst`.
fn new_icmp(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    a: Operand,
    b: Operand,
) -> Visit {
    let i1 = c.context.types.int(1);
    c.replace_with_new(inst, Opcode::ICmp(pred), i1, vec![a, b], Flags::default())
}

#[cfg(test)]
#[path = "combine_icmp_div_tests.rs"]
mod tests;
