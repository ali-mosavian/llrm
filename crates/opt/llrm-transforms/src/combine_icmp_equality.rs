//! InstCombine's `foldICmpEquality`
//! ([`InstCombineCompares.cpp:6222`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L6222))
//! at d1106deb71cc, up to the `ashr`/`trunc` range check.
//!
//! MIRROR: the order of the arms and each condition. Like the pin, the shift
//! pair returns `None` for the whole function when its amounts differ.
//!
//! OWED, in place as `// OWED:` comments: `foldICmpIntrinsicWithIntrinsic`,
//! the power-of-two canonicalizations, the `fshr` rotate, the `xor` constant
//! move, the add/sub/xor power-of-two mask and `foldICmpEqualityWithOffset`.
//!
//! DEVIATE: the constants are integers (MIR has no vectors), and a constant of
//! the `or` arm that is a constant expression does not match.
//!
//! # How it works
//! An equality compare whose operands share structure drops the shared part:
//! `(A^B) == A` is `B == 0`, `(X&Z) == (Y&Z)` is `((X^Y)&Z) == 0`, and a
//! masked value against a zero extension `(B & 255) == zext A` is the narrow
//! compare `A == trunc B`.
//!
//! # Example
//! The test of a byte against a wider value, as `strchr` writes it:
//!
//! ```text
//! %a = and i16 %c, 255          ; %t = trunc i16 %c to i8
//! %z = zext i8 %y to i16        ; %r = icmp eq i8 %y, %t
//! %r = icmp eq i16 %a, %z
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{binop, has_one_use, query, specific_int};

/// `new ICmpInst(pred, a, b)` in place of `inst`.
fn new_icmp(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    a: Operand,
    b: Operand,
) -> Option<Visit> {
    let i1 = c.context.types.int(1);
    Some(c.replace_with_new(inst, Opcode::ICmp(pred), i1, vec![a, b], Flags::default()))
}

/// `m_APInt`: the constant and its width.
fn apint(
    c: &Combiner,
    v: Operand,
) -> Option<(u128, u32)> {
    pattern::apint(&query(c, None), v)
}

/// The bit width of the integer `v`.
fn width_of(
    c: &Combiner,
    v: Operand,
) -> u32 {
    c.context.types.int_bits(c.type_of(v)).expect("an integer")
}

/// `APInt::isNegatedPowerOf2`: the top bit is set and the negation is a power
/// of two.
fn is_negated_power_of_two(
    bits: u128,
    width: u32,
) -> bool {
    let negated = bits.wrapping_neg() & mask(width);
    bits >> (width - 1) & 1 == 1 && negated.is_power_of_two()
}

/// `foldICmpEquality`.
pub fn fold_icmp_equality(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if !matches!(pred, IntPredicate::Eq | IntPredicate::Ne) {
        return None;
    }
    let ty = c.type_of(op0);
    let width = c.context.types.int_bits(ty)?;
    let zero = c.int(ty, 0);

    if let Some((a, b, _)) = binop(&query(c, None), op0, BinaryOp::Xor) {
        // (A^B) == A  ->  B == 0
        if a == op1 || b == op1 {
            let other = if a == op1 { b } else { a };
            return new_icmp(c, inst, pred, other, zero);
        }
        if let Some((cc, d, _)) = binop(&query(c, None), op1, BinaryOp::Xor) {
            // A^c1 == C^c2 --> A == C^(c1^c2)
            if let (Some((c1, _)), Some((c2, _))) = (apint(c, b), apint(c, d))
                && has_one_use(c, op1)
            {
                let folded = c.int(ty, c1 ^ c2);
                let xor = c.create_binop(inst, BinaryOp::Xor, cc, folded, Flags::default());
                return new_icmp(c, inst, pred, a, xor);
            }
            // A^B == A^D -> B == D
            if a == cc {
                return new_icmp(c, inst, pred, b, d);
            }
            if a == d {
                return new_icmp(c, inst, pred, b, cc);
            }
            if b == cc {
                return new_icmp(c, inst, pred, a, d);
            }
            if b == d {
                return new_icmp(c, inst, pred, a, cc);
            }
        }
    }

    // A == (A^B)  ->  B == 0
    if let Some((a, b, _)) = binop(&query(c, None), op1, BinaryOp::Xor)
        && (a == op0 || b == op0)
    {
        let other = if a == op0 { b } else { a };
        return new_icmp(c, inst, pred, other, zero);
    }

    // (X&Z) == (Y&Z) -> (X^Y) & Z == 0
    if let (Some((a, b, _)), Some((cc, d, _))) =
        (binop(&query(c, None), op0, BinaryOp::And), binop(&query(c, None), op1, BinaryOp::And))
    {
        let shared = if a == cc {
            Some((b, d, a))
        } else if a == d {
            Some((b, cc, a))
        } else if b == cc {
            Some((a, d, b))
        } else if b == d {
            Some((a, cc, b))
        } else {
            None
        };
        if let Some((x, y, z)) = shared {
            // If X^Y is a negative power of two, then `icmp eq/ne (Z & NegP2),
            // 0` will fold to `icmp ult/uge Z, -NegP2` incurring no
            // additional instructions.
            let xor_is_negated_power_of_two = apint(c, x)
                .zip(apint(c, y))
                .is_some_and(|((c0, _), (c1, _))| is_negated_power_of_two((c0 ^ c1) & mask(width), width));
            // If either Op0/Op1 are both one use or X^Y will constant fold and
            // one of Op0/Op1 are one use, proceed. In those cases
            // we are instruction neutral but `icmp eq/ne A, 0` is
            // easier to analyze than `icmp eq/ne A, B`.
            let uses = usize::from(has_one_use(c, op0))
                + usize::from(has_one_use(c, op1))
                + usize::from({
                    let q = query(c, None);
                    pattern::imm_constant(&q, x) && pattern::imm_constant(&q, y)
                });
            if xor_is_negated_power_of_two || uses >= 2 {
                let xor = c.create_binop(inst, BinaryOp::Xor, x, y, Flags::default());
                let and = c.create_binop(inst, BinaryOp::And, xor, z, Flags::default());
                let zero = c.int(c.type_of(and), 0);
                return new_icmp(c, inst, pred, and, zero);
            }
        }
    }

    // (X | C) == (Y | C) --> (X ^ Y) & ~C == 0
    if let (Some((x, k, _)), Some((y, k2, _))) =
        (binop(&query(c, None), op0, BinaryOp::Or), binop(&query(c, None), op1, BinaryOp::Or))
        && has_one_use(c, op0)
        && has_one_use(c, op1)
        && k == k2
        && let Some((bits, _)) = apint(c, k)
    {
        let xor = c.create_binop(inst, BinaryOp::Xor, x, y, Flags::default());
        let not = c.int(ty, !bits);
        let and = c.create_binop(inst, BinaryOp::And, xor, not, Flags::default());
        let zero = c.int(c.type_of(and), 0);
        return new_icmp(c, inst, pred, and, zero);
    }

    // (B & (Pow2C-1)) == zext A --> A == trunc B
    if let Some((_, _, a)) = pattern::cast(&query(c, None), op1, CastOp::ZExt)
        && (has_one_use(c, op0) || has_one_use(c, op1))
        && let Some((b, k, _)) = binop(&query(c, None), op0, BinaryOp::And)
        && let Some((ones, _)) = pattern::low_bit_mask(&query(c, None), k)
        && ones.trailing_ones() == width_of(c, a)
    {
        let narrow = c.type_of(a);
        let trunc = c.create_cast(inst, CastOp::Trunc, b, narrow, Flags::default());
        return new_icmp(c, inst, pred, a, trunc);
    }

    // (A >> C) == (B >> C) --> (A^B) u< (1 << C), for lshr and ashr pairs.
    for shr in [BinaryOp::LShr, BinaryOp::AShr] {
        if let (Some((a, p1, _)), Some((b, p2, _))) =
            (binop(&query(c, None), op0, shr), binop(&query(c, None), op1, shr))
            && has_one_use(c, op0)
            && has_one_use(c, op1)
            && let (Some((amount1, _)), Some((amount2, _))) = (apint(c, p1), apint(c, p2))
        {
            if amount1 != amount2 {
                return None;
            }
            if amount1 < u128::from(width) && amount1 != 0 {
                let new_pred = if pred == IntPredicate::Ne { IntPredicate::Uge } else { IntPredicate::Ult };
                let xor = c.create_binop(inst, BinaryOp::Xor, a, b, Flags::default());
                let limit = c.int(ty, 1u128 << amount1);
                return new_icmp(c, inst, new_pred, xor, limit);
            }
        }
    }

    // (A << C) == (B << C) --> ((A^B) & (~0U >> C)) == 0
    if let (Some((a, k, _)), Some((b, k2, _))) =
        (binop(&query(c, None), op0, BinaryOp::Shl), binop(&query(c, None), op1, BinaryOp::Shl))
        && has_one_use(c, op0)
        && has_one_use(c, op1)
        && k == k2
        && let Some((amount, _)) = apint(c, k)
        && amount < u128::from(width)
        && amount != 0
    {
        let xor = c.create_binop(inst, BinaryOp::Xor, a, b, Flags::default());
        let keep = c.int(ty, mask(width - amount as u32));
        let and = c.create_binop(inst, BinaryOp::And, xor, keep, Flags::default());
        return new_icmp(c, inst, pred, and, zero);
    }

    // icmp eq (trunc (lshr(X, cst1)), cst) --> icmp (and X, mask), cst
    if has_one_use(c, op0)
        && let Some((_, _, shifted)) = pattern::cast(&query(c, None), op0, CastOp::Trunc)
        && has_one_use(c, shifted)
        && let Some((a, k, _)) = binop(&query(c, None), shifted, BinaryOp::LShr)
        && let Some((amount, _)) = apint(c, k)
        && let Some((cst, _)) = apint(c, op1)
        // Only do this when A has multiple uses. This is most important to do
        // when it exposes other optimizations.
        && !has_one_use(c, a)
        && amount < u128::from(width_of(c, a))
    {
        let wide = width_of(c, a);
        let aty = c.type_of(a);
        let keep = c.int(aty, (mask(width) << amount) & mask(wide));
        let and = c.create_binop(inst, BinaryOp::And, a, keep, Flags::default());
        let compared = c.int(aty, (cst << amount) & mask(wide));
        return new_icmp(c, inst, pred, and, compared);
    }

    // OWED: foldICmpIntrinsicWithIntrinsic.

    // icmp eq (ashr (trunc A), BW-1), (trunc (lshr A, BW)) checks that the top
    // BW + 1 bits are all the same: A >=s INT_MIN && A <=s INT_MAX.
    if let Some((_, _, low)) = binop(&query(c, None), op0, BinaryOp::AShr).and_then(|(truncated, k, _)| {
        specific_int(&query(c, None), k, u128::from(width) - 1)
            .then(|| pattern::cast(&query(c, None), truncated, CastOp::Trunc))
            .flatten()
    }) && let Some((_, _, shifted)) = pattern::cast(&query(c, None), op1, CastOp::Trunc)
        && let Some((a, k, _)) = binop(&query(c, None), shifted, BinaryOp::LShr)
        && a == low
        && specific_int(&query(c, None), k, u128::from(width))
        && width_of(c, a) == width * 2
        && (has_one_use(c, op0) || has_one_use(c, op1))
    {
        let wide_ty = c.type_of(a);
        let bit = 1u128 << (width - 1);
        let offset = c.int(wide_ty, bit);
        let add = c.create_binop(inst, BinaryOp::Add, a, offset, Flags::default());
        let new_pred = if pred == IntPredicate::Eq { IntPredicate::Ult } else { IntPredicate::Uge };
        let limit = c.int(wide_ty, bit << 1);
        return new_icmp(c, inst, new_pred, add, limit);
    }

    // OWED: the power-of-two canonicalizations (`A & P2 != P2`), the `fshr`
    // rotate, `icmp (A ^ Cst), B` to `icmp (A ^ B), Cst`, the add/sub/xor
    // power-of-two mask and `foldICmpEqualityWithOffset`.
    None
}

#[cfg(test)]
#[path = "combine_icmp_equality_tests.rs"]
mod tests;
