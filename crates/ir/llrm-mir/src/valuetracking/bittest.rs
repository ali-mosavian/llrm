//! `decomposeBitTest`: a compare or a bit extraction as `(X & Mask) Pred C`.
//!
//! Translation of
//! [`decomposeBitTest`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CmpInstAnalysis.cpp#L174)
//! and `decomposeBitTestICmp`
//! ([L78](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CmpInstAnalysis.cpp#L78))
//! at d1106deb71cc.
//!
//! # Example
//! `icmp slt i16 %x, 0` is `(%x & 0x8000) != 0`; `icmp ult i16 %x, 16` is
//! `(%x & 0xfff0) == 0`.
//!
//! # Used by
//! `simplifySelectWithBitTest` (`simplify_select`, the defaults) and
//! InstCombine's masked-compare folds (`DecomposeAnd`, `AllowNonZeroC`).

use super::{Query, pattern};
use crate::context::mask;
use crate::module::Operand;
use crate::opcode::{BinaryOp, CastOp, IntPredicate};

/// `DecomposedBitTest`: `(X & Mask) Pred C`.
pub struct DecomposedBitTest {
    pub x: Operand,
    pub mask: u128,
    pub c: u128,
    pub pred: IntPredicate,
}

/// The flags of `decomposeBitTest`.
#[derive(Clone, Copy)]
pub struct BitTestOptions {
    pub look_through_trunc: bool,
    pub allow_non_zero_c: bool,
    pub decompose_and: bool,
}

impl BitTestOptions {
    /// The defaults of `decomposeBitTest`.
    pub const DEFAULT: Self = Self { look_through_trunc: true, allow_non_zero_c: false, decompose_and: false };
}

fn neg(
    value: u128,
    width: u32,
) -> u128 {
    value.wrapping_neg() & mask(width)
}

/// `APInt::isNegatedPowerOf2`.
fn is_negated_power_of_two(
    value: u128,
    width: u32,
) -> bool {
    neg(value, width).is_power_of_two()
}

/// `CmpInst::getStrictPredicate`.
fn strict(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Sge => IntPredicate::Sgt,
        IntPredicate::Sle => IntPredicate::Slt,
        IntPredicate::Uge => IntPredicate::Ugt,
        IntPredicate::Ule => IntPredicate::Ult,
        other => other,
    }
}

/// `decomposeBitTestICmp`.
fn decompose_icmp(
    q: &Query,
    mut lhs: Operand,
    rhs: Operand,
    mut pred: IntPredicate,
    options: BitTestOptions,
) -> Option<DecomposedBitTest> {
    if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) && !options.decompose_and {
        return None;
    }
    let (orig_c, width) = q.int_constant(rhs)?;
    let mut inverted = false;
    if matches!(
        pred,
        IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Sgt | IntPredicate::Sge
    ) {
        inverted = true;
        pred = pred.inverse();
    }
    let mut c = orig_c;
    if matches!(pred, IntPredicate::Ule | IntPredicate::Sle) {
        let max = if pred == IntPredicate::Sle { mask(width) >> 1 } else { mask(width) };
        if c == max {
            return None;
        }
        c = c.wrapping_add(1) & mask(width);
        pred = strict(pred);
    }
    let sign = 1u128 << (width - 1);
    let (bit_mask, bit_c, bit_pred) = match pred {
        IntPredicate::Slt => {
            // X < 0 is equivalent to (X & SignMask) != 0.
            if c == 0 {
                (sign, 0, IntPredicate::Ne)
            } else {
                let flipped = c ^ sign;
                if flipped.is_power_of_two() {
                    // X s< 10000100 is equivalent to (X & 11111100 == 10000000)
                    (neg(flipped, width), sign, IntPredicate::Eq)
                } else if is_negated_power_of_two(flipped, width) {
                    // X s< 01111100 is equivalent to (X & 11111100 != 01111100)
                    (flipped, c, IntPredicate::Ne)
                } else {
                    return None;
                }
            }
        }
        IntPredicate::Ult => {
            if c.is_power_of_two() {
                // X <u 2^n is equivalent to (X & ~(2^n-1)) == 0.
                (neg(c, width), 0, IntPredicate::Eq)
            } else if is_negated_power_of_two(c, width) {
                // X u< 11111100 is equivalent to (X & 11111100 != 11111100)
                (c, c, IntPredicate::Ne)
            } else {
                return None;
            }
        }
        IntPredicate::Eq | IntPredicate::Ne => {
            if let Some((_, _, value, and_mask)) = pattern::binary(q, lhs, BinaryOp::And)
                && let Some((and_c, _)) = q.int_constant(and_mask)
            {
                lhs = value;
                (and_c, c, pred)
            } else if options.look_through_trunc && pattern::cast(q, lhs, CastOp::Trunc).is_some() {
                // (trunc X) eq/ne C is (X & Mask) eq/ne C.
                (mask(width), c, pred)
            } else {
                return None;
            }
        }
        _ => return None,
    };
    if !options.allow_non_zero_c && bit_c != 0 {
        return None;
    }
    let bit_pred = if inverted { bit_pred.inverse() } else { bit_pred };
    if options.look_through_trunc
        && let Some((_, _, x)) = pattern::cast(q, lhs, CastOp::Trunc)
    {
        return Some(DecomposedBitTest { x, mask: bit_mask, c: bit_c, pred: bit_pred });
    }
    Some(DecomposedBitTest { x: lhs, mask: bit_mask, c: bit_c, pred: bit_pred })
}

/// `decomposeBitTest`.
pub fn decompose_bit_test(
    q: &Query,
    cond: Operand,
    options: BitTestOptions,
) -> Option<DecomposedBitTest> {
    if let Some((_, _, pred, lhs, rhs)) = pattern::icmp(q, cond) {
        // Don't allow pointers.
        if q.is_pointer(lhs) {
            return None;
        }
        return decompose_icmp(q, lhs, rhs, pred.pred, options);
    }
    if q.scalar_bits(cond) == 1 {
        if let Some((_, _, x)) = pattern::cast(q, cond, CastOp::Trunc) {
            return Some(DecomposedBitTest { x, mask: 1, c: 0, pred: IntPredicate::Ne });
        }
        if let Some(inner) = pattern::not(q, cond)
            && let Some((_, _, x)) = pattern::cast(q, inner, CastOp::Trunc)
        {
            return Some(DecomposedBitTest { x, mask: 1, c: 0, pred: IntPredicate::Eq });
        }
    }
    None
}
