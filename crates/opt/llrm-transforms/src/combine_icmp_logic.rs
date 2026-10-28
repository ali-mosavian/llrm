//! `foldBooleanAndOr`: an `and` or `or` of two `i1` compares, folded to one
//! compare.
//!
//! Translation of
//! [`foldBooleanAndOr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L3685)
//! at d1106deb71cc: `foldLogOpOfMaskedICmps`
//! ([L536](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L536))
//! with `getMaskedTypeForICmpPair`
//! ([L215](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L215))
//! and its two helpers, then the part of `foldAndOrOfICmps`
//! ([L3445](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L3445))
//! listed below.
//!
//! # How it works
//! Both operands are `i1` compares. `getMaskedTypeForICmpPair` views each as
//! `(A & B) ==/!= C` (a compare of a mask, a bit test, or a compare with an
//! all-ones mask), finds the `A` they share and classifies each side
//! (all-ones, all-zeros, mixed). The classes both sides satisfy say how to
//! merge: OR the masks, AND them, or one side implies the other. The part of
//! `foldAndOrOfICmps` kept merges two compares of the same operands by their
//! compare codes and turns pairs of zero, all-ones and sign tests into one
//! test of the OR, AND or XOR.
//!
//! # Example
//! A sign test of each of two values is one test of their `or`
//! (`(icmp slt a, 0)` is `(a & signbit) != 0`, so the sign bit is the common
//! `A` of the masked form):
//!
//! ```text
//! %x = icmp slt i16 %a, 0                     %o = or i16 %a, %b
//! %y = icmp slt i16 %b, 0           ==>       %r = icmp slt i16 %o, 0
//! %r = or i1 %x, %y
//! ```
//!
//! MIRROR: the C++ order and the one-use and poison-safety conditions.
//!
//! DEVIATE 1: the `isNaN` idiom of `foldLogOpOfMaskedICmps_NotAllZeros_BMask_
//! Mixed` (a floating-point class test) is not translated.
//!
//! DEVIATE 2: of `foldAndOrOfICmps` only the compare-code merge, the sign-bit
//! pair, `(ne A, 0) | (ne B, 0)` and `(ne A, -1) | (ne B, -1)` are here. Not
//! yet: `foldAndOrOfICmpEqConstantAndICmp`, `foldAndOrOfICmpsWithConstEq`,
//! `foldIsPowerOf2OrZero`, `foldSignedTruncationCheck`,
//! `foldIsPowerOf2`, `foldPowerOf2AndShiftedMask`,
//! `foldUnsignedUnderflowCheck`, `foldAndOrOfICmpsWithPow2AndWithZero`, the
//! `trunc`/`and` fold and the `isnormal` fold; `simplifyRangeCheck` and
//! `foldAndOrOfICmpsUsingRanges` are in `combine_icmp_ranges`; also
//! `foldLogicOfFCmps` and `foldEqOfParts`.
//!
//! `reassociateBooleanAndOr` is here: `visit_and` and `visit_or` call it after
//! the fold of the pair.
//!
//! # Used by
//! `visit_and` and `visit_or` (the bitwise forms).

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::bittest::{BitTestOptions, decompose_bit_test};
use llrm_mir::valuetracking::fp::is_sign_bit_check;
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison;
use llrm_mir::valuetracking::power2::is_known_to_be_a_power_of_two;
use llrm_mir::valuetracking::{Query, pattern};

use crate::combine::Combiner;
use crate::combine_icmp_ranges::{fold_and_or_of_icmps_using_ranges, simplify_range_check};
use crate::combine_shared::{binop, drop_poison_generating_flags, has_one_use, query};

const A_MASK_ALL_ONES: u32 = 1;
const A_MASK_NOT_ALL_ONES: u32 = 2;
const B_MASK_ALL_ONES: u32 = 4;
const B_MASK_NOT_ALL_ONES: u32 = 8;
const MASK_ALL_ZEROS: u32 = 16;
const MASK_NOT_ALL_ZEROS: u32 = 32;
const A_MASK_MIXED: u32 = 64;
const A_MASK_NOT_MIXED: u32 = 128;
const B_MASK_MIXED: u32 = 256;
const B_MASK_NOT_MIXED: u32 = 512;

fn constant(
    q: &Query,
    v: Operand,
) -> Option<u128> {
    q.int_constant(v).map(|(bits, _)| bits)
}

fn is_equality(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
}

fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Slt | IntPredicate::Sle | IntPredicate::Sgt | IntPredicate::Sge
    )
}

/// `getMaskedICmpType`: the patterns `icmp pred (A & B), C` satisfies.
fn masked_icmp_type(
    q: &Query,
    a: Operand,
    b: Operand,
    c: Operand,
    pred: IntPredicate,
) -> u32 {
    let (const_a, const_b, const_c) = (constant(q, a), constant(q, b), constant(q, c));
    let is_eq = pred == IntPredicate::Eq;
    let is_a_pow2 = const_a.is_some_and(u128::is_power_of_two);
    let is_b_pow2 = const_b.is_some_and(u128::is_power_of_two);
    let pick = |eq: u32, ne: u32| if is_eq { eq } else { ne };
    let mut mask_val = 0;
    if const_c == Some(0) {
        // If C is zero, both A and B qualify as the mask.
        mask_val |= pick(
            MASK_ALL_ZEROS | A_MASK_MIXED | B_MASK_MIXED,
            MASK_NOT_ALL_ZEROS | A_MASK_NOT_MIXED | B_MASK_NOT_MIXED,
        );
        if is_a_pow2 {
            mask_val |= pick(A_MASK_NOT_ALL_ONES | A_MASK_NOT_MIXED, A_MASK_ALL_ONES | A_MASK_MIXED);
        }
        if is_b_pow2 {
            mask_val |= pick(B_MASK_NOT_ALL_ONES | B_MASK_NOT_MIXED, B_MASK_ALL_ONES | B_MASK_MIXED);
        }
        return mask_val;
    }
    if a == c {
        mask_val |= pick(A_MASK_ALL_ONES | A_MASK_MIXED, A_MASK_NOT_ALL_ONES | A_MASK_NOT_MIXED);
        if is_a_pow2 {
            mask_val |= pick(MASK_NOT_ALL_ZEROS | A_MASK_NOT_MIXED, MASK_ALL_ZEROS | A_MASK_MIXED);
        }
    } else if let (Some(x), Some(y)) = (const_a, const_c)
        && y & x == y
    {
        mask_val |= pick(A_MASK_MIXED, A_MASK_NOT_MIXED);
    }
    if b == c {
        mask_val |= pick(B_MASK_ALL_ONES | B_MASK_MIXED, B_MASK_NOT_ALL_ONES | B_MASK_NOT_MIXED);
        if is_b_pow2 {
            mask_val |= pick(MASK_NOT_ALL_ZEROS | B_MASK_NOT_MIXED, MASK_ALL_ZEROS | B_MASK_MIXED);
        }
    } else if let (Some(x), Some(y)) = (const_b, const_c)
        && y & x == y
    {
        mask_val |= pick(B_MASK_MIXED, B_MASK_NOT_MIXED);
    }
    mask_val
}

/// `conjugateICmpMask`: the analysis if every comparison had the other sense.
fn conjugate_icmp_mask(mask: u32) -> u32 {
    ((mask & (A_MASK_ALL_ONES | B_MASK_ALL_ONES | MASK_ALL_ZEROS | A_MASK_MIXED | B_MASK_MIXED)) << 1)
        | ((mask
            & (A_MASK_NOT_ALL_ONES | B_MASK_NOT_ALL_ONES | MASK_NOT_ALL_ZEROS | A_MASK_NOT_MIXED | B_MASK_NOT_MIXED))
            >> 1)
}

/// The operands `getMaskedTypeForICmpPair` finds: `(icmp (A & B) PredL C)` and
/// `(icmp (A & D) PredR E)`, and the pattern classes of each.
struct MaskedPair {
    a: Operand,
    b: Operand,
    c: Operand,
    d: Operand,
    e: Operand,
    pred_l: IntPredicate,
    pred_r: IntPredicate,
    left: u32,
    right: u32,
}

/// The local `decomposeBitTest`: `(pred, X, Mask, C)` with the constants as
/// values of `X`'s type.
fn decompose(
    c: &Combiner,
    q: &Query,
    cond: Operand,
) -> Option<(IntPredicate, Operand, Operand, Operand)> {
    let options = BitTestOptions { look_through_trunc: true, allow_non_zero_c: true, decompose_and: true };
    let found = decompose_bit_test(q, cond, options)?;
    let ty = c.type_of(found.x);
    Some((found.pred, found.x, c.int(ty, found.mask), c.int(ty, found.c)))
}

fn is_int(
    c: &Combiner,
    v: Operand,
) -> bool {
    c.context.types.int_bits(c.type_of(v)).is_some()
}

/// `m_And(m_Value(X), m_Value(Y))` of `v`, else `v` masked with all ones.
fn trivially_masked(
    c: &Combiner,
    q: &Query,
    v: Operand,
) -> (Operand, Operand) {
    match pattern::binary(q, v, BinaryOp::And) {
        Some((_, _, x, y)) => (x, y),
        None => (v, c.all_ones(c.type_of(v))),
    }
}

/// `getMaskedTypeForICmpPair`.
fn masked_type_for_icmp_pair(
    c: &Combiner,
    q: &Query,
    lhs: Operand,
    rhs: Operand,
) -> Option<MaskedPair> {
    // Check whether the icmp can be decomposed into a bit test.
    let (pred_l, l11, l12, l2, l1, l21, l22);
    if let Some((pred, x, y, z)) = decompose(c, q, lhs) {
        (pred_l, l11, l12, l2) = (pred, x, y, z);
        (l1, l21, l22) = (None, None, None);
    } else {
        let (_, _, pred, op0, op1) = pattern::icmp(q, lhs)?;
        // Don't allow pointers.
        if !is_int(c, op0) {
            return None;
        }
        pred_l = pred.pred;
        l2 = op1;
        l1 = Some(op0);
        (l11, l12) = trivially_masked(c, q, op0);
        let (x, y) = trivially_masked(c, q, op1);
        (l21, l22) = (Some(x), Some(y));
    }
    // Bail if LHS was an icmp that can't be decomposed into an equality.
    if !is_equality(pred_l) {
        return None;
    }
    let in_left = |v: Operand| [Some(l11), Some(l12), l21, l22].contains(&Some(v));

    let (a, d, e, pred_r);
    if let Some((pred, r11, r12, r2)) = decompose(c, q, rhs) {
        pred_r = pred;
        if in_left(r11) {
            (a, d) = (r11, r12);
        } else if in_left(r12) {
            (a, d) = (r12, r11);
        } else {
            return None;
        }
        e = r2;
    } else {
        let (_, _, pred, r1, r2) = pattern::icmp(q, rhs)?;
        if !is_int(c, r1) {
            return None;
        }
        pred_r = pred.pred;
        let (r11, r12) = trivially_masked(c, q, r1);
        let mut found = None;
        if in_left(r11) {
            found = Some((r11, r12, r2));
        } else if in_left(r12) {
            found = Some((r12, r11, r2));
        }
        // Avoid matching against the -1 value made for an unmasked operand.
        if let Some((x, ..)) = found
            && q.int_constant(x).is_some_and(|(bits, width)| bits == mask(width))
        {
            found = None;
        }
        // Look for ANDs on the right side of the RHS icmp.
        let (x, y, z) = match found {
            Some(found) => found,
            None => {
                let (r11, r12) = trivially_masked(c, q, r2);
                if in_left(r11) {
                    (r11, r12, r1)
                } else if in_left(r12) {
                    (r12, r11, r1)
                } else {
                    return None;
                }
            }
        };
        (a, d, e) = (x, y, z);
    }
    // Bail if RHS was an icmp that can't be decomposed into an equality.
    if !is_equality(pred_r) {
        return None;
    }
    let (b, c_op) = if l11 == a {
        (l12, l2)
    } else if l12 == a {
        (l11, l2)
    } else if l21 == Some(a) {
        (l22?, l1?)
    } else if l22 == Some(a) {
        (l21?, l1?)
    } else {
        return None;
    };
    let left = masked_icmp_type(q, a, b, c_op, pred_l);
    let right = masked_icmp_type(q, a, d, e, pred_r);
    Some(MaskedPair { a, b, c: c_op, d, e, pred_l, pred_r, left, right })
}

/// `ConstantInt::get(LHS->getType(), !IsAnd)`.
fn truth(
    c: &Combiner,
    lhs: Operand,
    value: bool,
) -> Operand {
    c.int(c.type_of(lhs), u128::from(value))
}

/// The `samesign` flag of a compare, which a fold that returns it unchanged
/// cannot promise.
fn drop_samesign(
    c: &mut Combiner,
    v: Operand,
) {
    let found = query(c, None).instruction_of(v).map(|(inst, instruction)| (inst, instruction.flags));
    if let Some((inst, flags)) = found
        && flags.contains(Flags::SAMESIGN)
    {
        c.function.set_flags(inst, flags.without(Flags::SAMESIGN));
    }
}

/// `foldLogOpOfMaskedICmps_NotAllZeros_BMask_Mixed`: `(icmp ne (A & B), 0) &
/// (icmp eq (A & D), E)` with `D & E == E`, or its negation.
#[allow(clippy::too_many_arguments)]
fn fold_not_all_zeros_bmask_mixed(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
    is_and: bool,
    (a, b, d, e): (Operand, Operand, Operand, Operand),
    pred_r: IntPredicate,
) -> Option<Operand> {
    let (b_cst, d_cst, orig_e) = {
        let q = query(c, Some(inst));
        (constant(&q, b)?, constant(&q, d)?, constant(&q, e)?)
    };
    let new_cc = if is_and { IntPredicate::Eq } else { IntPredicate::Ne };
    // Update E to the canonical form when D is a power of two and RHS is
    // canonicalized as (icmp ne (A & D), 0) -> (icmp eq (A & D), D) or
    // (icmp ne (A & D), D) -> (icmp eq (A & D), 0).
    let e_cst = if pred_r != new_cc { orig_e ^ d_cst } else { orig_e };
    // If B or D is zero, the compares fold by other rules.
    if b_cst == 0 || d_cst == 0 {
        return None;
    }
    // B and D that do not intersect: the isNaN idiom, or nothing.
    // DEVIATE 1: the isNaN idiom.
    if b_cst & d_cst == 0 {
        return None;
    }
    // B has one bit D does not, and RHS says the rest of B is zero: that bit
    // must be set, so (A & (B | D)) == (B & (B ^ D)) | E.
    if (b_cst & d_cst) & e_cst == 0 && (b_cst & (b_cst ^ d_cst)).is_power_of_two() {
        let ty = c.type_of(a);
        let new_mask = c.int(ty, b_cst | d_cst);
        let new_value = c.int(ty, (b_cst & (b_cst ^ d_cst)) | e_cst);
        let new_and = c.create_binop(inst, BinaryOp::And, a, new_mask, Flags::default());
        return Some(c.create_icmp(inst, new_cc, new_and, new_value));
    }
    let sub_set = |x: u128, y: u128| x & y == x;
    let super_set = |x: u128, y: u128| x & y == y;
    // B a superset of D, a subset of D, or B == D; else nothing to deduce.
    if !sub_set(b_cst, d_cst) && !super_set(b_cst, d_cst) {
        return None;
    }
    // E zero: B within D contradicts, the whole is false (true if negated).
    if e_cst == 0 {
        return sub_set(b_cst, d_cst).then(|| truth(c, lhs, !is_and));
    }
    // B a superset of D and E not zero: RHS implies LHS.
    if super_set(b_cst, d_cst) {
        drop_samesign(c, rhs);
        return Some(rhs);
    }
    // B a subset of D with a bit in common with E: LHS is subsumed by RHS.
    if b_cst & e_cst != 0 {
        drop_samesign(c, rhs);
        return Some(rhs);
    }
    // Otherwise they contradict.
    Some(truth(c, lhs, !is_and))
}

/// `foldLogOpOfMaskedICmpsAsymmetric`.
fn fold_masked_icmps_asymmetric(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
    is_and: bool,
    m: &MaskedPair,
) -> Option<Operand> {
    let (mut left, mut right) = (m.left, m.right);
    if !is_and {
        left = conjugate_icmp_mask(left);
        right = conjugate_icmp_mask(right);
    }
    if left & MASK_NOT_ALL_ZEROS != 0 && right & B_MASK_MIXED != 0 {
        fold_not_all_zeros_bmask_mixed(c, inst, lhs, rhs, is_and, (m.a, m.b, m.d, m.e), m.pred_r)
    } else if left & B_MASK_MIXED != 0 && right & MASK_NOT_ALL_ZEROS != 0 {
        fold_not_all_zeros_bmask_mixed(c, inst, rhs, lhs, is_and, (m.a, m.d, m.b, m.c), m.pred_l)
    } else {
        None
    }
}

/// `foldLogOpOfMaskedICmps`: `(icmp (A & B) ==/!= C) &/| (icmp (A & D) ==/!=
/// E)` as one `(icmp (A & X) ==/!= Y)`.
fn fold_log_op_of_masked_icmps(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
    is_and: bool,
    is_logical: bool,
) -> Option<Operand> {
    let m = {
        let q = query(c, Some(inst));
        masked_type_for_icmp_pair(c, &q, lhs, rhs)?
    };
    let mut mask_set = m.left & m.right;
    if mask_set == 0 {
        // Even if the two sides share no pattern, a fold may still happen.
        return fold_masked_icmps_asymmetric(c, inst, lhs, rhs, is_and, &m);
    }
    // (icmp (A & B) Op C) | (icmp (A & D) Op E) is ![ (icmp (A & B) !Op C) &
    // (icmp (A & D) !Op E) ]: the conjunction with every sense flipped.
    let new_cc = if is_and { IntPredicate::Eq } else { IntPredicate::Ne };
    if !is_and {
        mask_set = conjugate_icmp_mask(mask_set);
    }
    let (a, b, d) = (m.a, m.b, m.d);
    let a_ty = c.type_of(a);
    let poison_safe =
        |c: &Combiner, v: Operand| !is_logical || is_guaranteed_not_to_be_undef_or_poison(&query(c, Some(inst)), v, 0);

    if mask_set & MASK_ALL_ZEROS != 0 {
        // (icmp eq (A & B), 0) & (icmp eq (A & D), 0) -> (icmp eq (A & (B|D)),
        // 0)
        if !poison_safe(c, d) {
            return None;
        }
        let new_or = c.create_binop(inst, BinaryOp::Or, b, d, Flags::default());
        let new_and = c.create_binop(inst, BinaryOp::And, a, new_or, Flags::default());
        // Not C as zero: (icmp ne (A & B), B) & (icmp ne (A & D), D) with B and
        // D of one bit each is handled here too.
        let zero = c.int(a_ty, 0);
        return Some(c.create_icmp(inst, new_cc, new_and, zero));
    }
    if mask_set & B_MASK_ALL_ONES != 0 {
        // (icmp eq (A & B), B) & (icmp eq (A & D), D) -> (icmp eq (A & (B|D)),
        // (B|D))
        if !poison_safe(c, d) {
            return None;
        }
        let new_or = c.create_binop(inst, BinaryOp::Or, b, d, Flags::default());
        let new_and = c.create_binop(inst, BinaryOp::And, a, new_or, Flags::default());
        return Some(c.create_icmp(inst, new_cc, new_and, new_or));
    }
    if mask_set & A_MASK_ALL_ONES != 0 {
        // (icmp eq (A & B), A) & (icmp eq (A & D), A) -> (icmp eq (A & (B&D)),
        // A)
        if !poison_safe(c, d) {
            return None;
        }
        let new_and1 = c.create_binop(inst, BinaryOp::And, b, d, Flags::default());
        let new_and2 = c.create_binop(inst, BinaryOp::And, a, new_and1, Flags::default());
        return Some(c.create_icmp(inst, new_cc, new_and2, a));
    }

    let consts = {
        let q = query(c, Some(inst));
        (constant(&q, b), constant(&q, d), constant(&q, m.c), constant(&q, m.e))
    };
    if let (Some(const_b), Some(const_d), old_c, old_e) = consts {
        if mask_set & (MASK_NOT_ALL_ZEROS | B_MASK_NOT_ALL_ONES) != 0 {
            // (icmp ne (A & B), 0) & (icmp ne (A & D), 0) and
            // (icmp ne (A & B), B) & (icmp ne (A & D), D) are the one with the
            // smaller mask... of B and D, if one is a superset of the other.
            let new_mask = const_b & const_d;
            if new_mask == const_b {
                return Some(lhs);
            }
            if new_mask == const_d {
                if is_logical && let Some((rhs_inst, _)) = query(c, Some(inst)).instruction_of(rhs) {
                    drop_poison_generating_flags(c, rhs_inst);
                }
                return Some(rhs);
            }
        }
        if mask_set & A_MASK_NOT_ALL_ONES != 0 {
            // (icmp ne (A & B), B) & (icmp ne (A & D), D) -> (icmp ne (A & B),
            // A) or (icmp ne (A & D), A), where B|D is B or D.
            let new_mask = const_b | const_d;
            if new_mask == const_b {
                return Some(lhs);
            }
            if new_mask == const_d {
                return Some(rhs);
            }
        }
        if mask_set & (B_MASK_MIXED | B_MASK_NOT_MIXED) != 0 {
            // Mixed: (icmp eq (A & B), C) & (icmp eq (A & D), E) with
            // (B & D) & (C ^ E) == 0 is (icmp eq (A & (B|D)), (C|E)).
            // NotMixed: (icmp ne (A & B), C) & (icmp ne (A & D), E) with B or D
            // within the other is (icmp ne (A & (B & D)), (C & E)).
            let (old_c, old_e) = (old_c?, old_e?);
            let fold_b_mixed = |c: &mut Combiner, cc: IntPredicate, is_not: bool| -> Option<Operand> {
                let cc = if is_not { cc.inverse() } else { cc };
                let const_c = if m.pred_l != cc { const_b ^ old_c } else { old_c };
                let const_e = if m.pred_r != cc { const_d ^ old_e } else { old_e };
                if (const_b & const_d) & (const_c ^ const_e) != 0 {
                    return if is_not { None } else { Some(truth(c, lhs, !is_and)) };
                }
                let sub_set = |x: u128, y: u128| x & y == x;
                if is_not && !sub_set(const_b, const_d) && !sub_set(const_d, const_b) {
                    return None;
                }
                let (bd, ce) = if is_not {
                    (const_b & const_d, const_c & const_e)
                } else {
                    (const_b | const_d, const_c | const_e)
                };
                let bd_value = c.int(a_ty, bd);
                let new_and = c.create_binop(inst, BinaryOp::And, a, bd_value, Flags::default());
                let ce_value = c.int(a_ty, ce);
                Some(c.create_icmp(inst, cc, new_and, ce_value))
            };
            if mask_set & B_MASK_MIXED != 0 {
                return fold_b_mixed(c, new_cc, false);
            }
            return fold_b_mixed(c, new_cc, true);
        }
    }

    // (icmp eq (A & B), 0) | (icmp eq (A & D), 0) -> (icmp ne (A & (B|D)),
    // (B|D)) (icmp ne (A & B), 0) & (icmp ne (A & D), 0) -> (icmp eq (A &
    // (B|D)), (B|D)) if B and D are known to be a power of two.
    if mask_set & MASK_NOT_ALL_ZEROS != 0 {
        let both = {
            let q = query(c, Some(inst));
            is_known_to_be_a_power_of_two(&q, b, false, 0) && is_known_to_be_a_power_of_two(&q, d, false, 0)
        };
        if both {
            // A logical and/or must not pass on poison from the RHS: freeze.
            let d = if is_logical {
                let freeze = c.insert_before(inst, Opcode::Freeze, c.type_of(d), vec![d], Flags::default());
                Operand::Value(c.function.instruction(freeze).result.expect("a freeze's value"))
            } else {
                d
            };
            let mask_value = c.create_binop(inst, BinaryOp::Or, b, d, Flags::default());
            let masked = c.create_binop(inst, BinaryOp::And, a, mask_value, Flags::default());
            return Some(c.create_icmp(inst, new_cc, masked, mask_value));
        }
    }
    None
}

/// `getICmpCode`.
fn icmp_code(pred: IntPredicate) -> u32 {
    match pred {
        IntPredicate::Ugt | IntPredicate::Sgt => 1,
        IntPredicate::Eq => 2,
        IntPredicate::Uge | IntPredicate::Sge => 3,
        IntPredicate::Ult | IntPredicate::Slt => 4,
        IntPredicate::Ne => 5,
        IntPredicate::Ule | IntPredicate::Sle => 6,
    }
}

/// `predicatesFoldable`.
fn predicates_foldable(
    p1: IntPredicate,
    p2: IntPredicate,
) -> bool {
    is_signed(p1) == is_signed(p2) || (is_signed(p1) && is_equality(p2)) || (is_signed(p2) && is_equality(p1))
}

/// `getNewICmpValue`: `getPredForICmpCode`, then the compare.
fn new_icmp_value(
    c: &mut Combiner,
    inst: InstId,
    code: u32,
    sign: bool,
    lhs: Operand,
    rhs: Operand,
) -> Operand {
    let pick = |signed: IntPredicate, unsigned: IntPredicate| if sign { signed } else { unsigned };
    let pred = match code {
        0 | 7 => {
            let i1 = c.context.types.int(1);
            return c.int(i1, u128::from(code == 7));
        }
        1 => pick(IntPredicate::Sgt, IntPredicate::Ugt),
        2 => IntPredicate::Eq,
        3 => pick(IntPredicate::Sge, IntPredicate::Uge),
        4 => pick(IntPredicate::Slt, IntPredicate::Ult),
        5 => IntPredicate::Ne,
        _ => pick(IntPredicate::Sle, IntPredicate::Ule),
    };
    c.create_icmp(inst, pred, lhs, rhs)
}

/// `foldAndOrOfICmps`, the part DEVIATE 2 leaves in.
fn fold_and_or_of_icmps(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
    is_and: bool,
    is_logical: bool,
) -> Option<Operand> {
    let (pred_l, mut lhs0, mut lhs1, pred_r, rhs0, rhs1, lhs_c, rhs_c, width, rhs_not_poison) = {
        let q = query(c, Some(inst));
        let (_, _, pred_l, lhs0, lhs1) = pattern::icmp(&q, lhs)?;
        let (_, _, pred_r, rhs0, rhs1) = pattern::icmp(&q, rhs)?;
        let rhs_not_poison = !is_logical || is_guaranteed_not_to_be_undef_or_poison(&q, rhs0, 0);
        (
            pred_l.pred,
            lhs0,
            lhs1,
            pred_r.pred,
            rhs0,
            rhs1,
            constant(&q, lhs1),
            constant(&q, rhs1),
            q.scalar_bits(lhs0),
            rhs_not_poison,
        )
    };
    let mut pred_lhs = pred_l;
    let (lhs_one_use, rhs_one_use) = (has_one_use(c, lhs), has_one_use(c, rhs));

    // (icmp1 A, B) | (icmp2 A, B) --> (icmp3 A, B), and likewise for and.
    if predicates_foldable(pred_lhs, pred_r) {
        if lhs0 == rhs1 && lhs1 == rhs0 {
            pred_lhs = pred_lhs.swapped();
            std::mem::swap(&mut lhs0, &mut lhs1);
        }
        if lhs0 == rhs0 && lhs1 == rhs1 {
            let (code_l, code_r) = (icmp_code(pred_lhs), icmp_code(pred_r));
            let code = if is_and { code_l & code_r } else { code_l | code_r };
            let sign = is_signed(pred_lhs) || is_signed(pred_r);
            return Some(new_icmp_value(c, inst, code, sign, lhs0, lhs1));
        }
    }

    let same_type = c.type_of(lhs0) == c.type_of(rhs0);
    let ones = mask(width);
    // (icmp ne A, 0) | (icmp ne B, 0) --> (icmp ne (A|B), 0)
    // (icmp eq A, 0) & (icmp eq B, 0) --> (icmp eq (A|B), 0)
    if pred_lhs == if is_and { IntPredicate::Eq } else { IntPredicate::Ne }
        && pred_lhs == pred_r
        && lhs_c == Some(0)
        && rhs_c == Some(0)
        && same_type
        && rhs_not_poison
    {
        let new_or = c.create_binop(inst, BinaryOp::Or, lhs0, rhs0, Flags::default());
        let zero = c.int(c.type_of(new_or), 0);
        return Some(c.create_icmp(inst, pred_lhs, new_or, zero));
    }
    // (icmp ne A, -1) | (icmp ne B, -1) --> (icmp ne (A&B), -1)
    // (icmp eq A, -1) & (icmp eq B, -1) --> (icmp eq (A&B), -1)
    if pred_lhs == if is_and { IntPredicate::Eq } else { IntPredicate::Ne }
        && pred_lhs == pred_r
        && lhs_c == Some(ones)
        && rhs_c == Some(ones)
        && same_type
        && rhs_not_poison
    {
        let new_and = c.create_binop(inst, BinaryOp::And, lhs0, rhs0, Flags::default());
        let all_ones = c.all_ones(c.type_of(lhs0));
        return Some(c.create_icmp(inst, pred_lhs, new_and, all_ones));
    }

    // (icmp slt x, 0) | (icmp sgt x, n) --> icmp ugt x, n
    // (icmp sge x, 0) & (icmp slt x, n) --> icmp ult x, n
    // Not for the select form: one of the two directions is poison-safe, the
    // other needs a freeze.
    if !is_logical {
        let (l, r) = ((lhs0, lhs1), (rhs0, rhs1));
        if let Some(found) = simplify_range_check(c, inst, pred_lhs, l.0, l.1, pred_r, r.0, r.1, !is_and) {
            return Some(found);
        }
        if let Some(found) = simplify_range_check(c, inst, pred_r, r.0, r.1, pred_lhs, l.0, l.1, !is_and) {
            return Some(found);
        }
    }

    // This only handles icmp of constants: (icmp1 A, C1) | (icmp2 B, C2).
    let (lhs_c, rhs_c) = (lhs_c?, rhs_c?);
    // Naive tests of whether two values share a sign, and the inverted form:
    // (icmp slt (X & Y), 0) | (icmp sgt (X | Y), -1) -> (icmp sgt (X ^ Y), -1)
    // (icmp slt (X | Y), 0) & (icmp sgt (X & Y), -1) -> (icmp slt (X ^ Y), 0)
    let sign_l = is_sign_bit_check(pred_lhs, lhs_c, width);
    let sign_r = is_sign_bit_check(pred_r, rhs_c, width);
    if let (Some(true_if_signed_l), Some(true_if_signed_r)) = (sign_l, sign_r)
        && (rhs_one_use || lhs_one_use)
    {
        let q = query(c, Some(inst));
        let pair = |left_op: BinaryOp, right_op: BinaryOp| -> Option<(Operand, Operand)> {
            let (_, _, x, y) = pattern::binary(&q, lhs0, left_op)?;
            let (a, b, _) = binop(&q, rhs0, right_op)?;
            ((a, b) == (x, y) || (a, b) == (y, x)).then_some((x, y))
        };
        let found = if is_and {
            if true_if_signed_l && !true_if_signed_r {
                pair(BinaryOp::Or, BinaryOp::And)
            } else if !true_if_signed_l && true_if_signed_r {
                pair(BinaryOp::And, BinaryOp::Or)
            } else {
                None
            }
        } else if true_if_signed_l && !true_if_signed_r {
            pair(BinaryOp::And, BinaryOp::Or)
        } else if !true_if_signed_l && true_if_signed_r {
            pair(BinaryOp::Or, BinaryOp::And)
        } else {
            None
        };
        if let Some((x, y)) = found {
            let xor = c.create_binop(inst, BinaryOp::Xor, x, y, Flags::default());
            let zero = c.int(c.type_of(xor), 0);
            // CreateIsNeg / CreateIsNotNeg
            return Some(if is_and {
                c.create_icmp(inst, IntPredicate::Slt, xor, zero)
            } else {
                let ones = c.all_ones(c.type_of(xor));
                c.create_icmp(inst, IntPredicate::Sgt, xor, ones)
            });
        }
    }
    fold_and_or_of_icmps_using_ranges(
        c,
        inst,
        pred_lhs,
        (lhs0, lhs1),
        pred_r,
        (rhs0, rhs1),
        (lhs_one_use, rhs_one_use),
        is_and,
    )
}

/// `foldBooleanAndOr`: the value `lhs &/| rhs` (of `i1`s) folds to, if any.
pub fn fold_boolean_and_or(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
    is_and: bool,
    is_logical: bool,
) -> Option<Operand> {
    if c.context.types.int_bits(c.type_of(lhs)) != Some(1) {
        return None;
    }
    // (icmp ne (A & B), C) | (icmp ne (A & D), E) and its dual.
    if let Some(found) = fold_log_op_of_masked_icmps(c, inst, lhs, rhs, is_and, is_logical) {
        return Some(found);
    }
    fold_and_or_of_icmps(c, inst, lhs, rhs, is_and, is_logical)
    // DEVIATE 2: foldLogicOfFCmps, foldEqOfParts.
}

/// `reassociateBooleanAndOr`: `lhs op (x lop y)` is `(lhs op x) lop y` or
/// `x lop (lhs op y)`, where a pair of them folds.
fn reassociate_boolean_and_or(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    (x, y): (Operand, Operand),
    is_and: bool,
    rhs_is_logical: bool,
) -> Option<Operand> {
    let join = |c: &mut Combiner, a: Operand, b: Operand| match (is_and, rhs_is_logical) {
        (true, true) => c.create_logical_and(inst, inst, a, b),
        (false, true) => c.create_logical_or(inst, inst, a, b),
        (true, false) => c.create_binop(inst, BinaryOp::And, a, b, Flags::default()),
        (false, false) => c.create_binop(inst, BinaryOp::Or, a, b, Flags::default()),
    };
    if let Some(folded) = fold_boolean_and_or(c, inst, lhs, x, is_and, false) {
        return Some(join(c, folded, y));
    }
    let folded = fold_boolean_and_or(c, inst, lhs, y, is_and, false)?;
    Some(join(c, x, folded))
}

/// The `reassociateBooleanAndOr` calls of `visitAnd` and `visitOr`: the
/// operand that is a one-use logical `op` of the same kind, the right one
/// first.
pub fn reassociate_boolean_operands(
    c: &mut Combiner,
    inst: InstId,
    (op0, op1): (Operand, Operand),
    is_and: bool,
) -> Option<Operand> {
    for (inner, other) in [(op1, op0), (op0, op1)] {
        let q = query(c, Some(inst));
        let parts = if is_and { pattern::logical_and(&q, inner) } else { pattern::logical_or(&q, inner) };
        let Some(parts) = parts.filter(|_| has_one_use(c, inner)) else { continue };
        let is_logical = c.select_of(inner).is_some();
        if let Some(found) = reassociate_boolean_and_or(c, inst, other, parts, is_and, is_logical) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
#[path = "combine_icmp_logic_tests.rs"]
mod tests;
