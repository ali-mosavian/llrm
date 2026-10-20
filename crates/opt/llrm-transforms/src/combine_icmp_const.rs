//! `icmp (binop ...), C`: the folds of a compare of a binary operator with a
//! constant, so far the `and` family.
//!
//! Translation, from
//! [`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)
//! at d1106deb71cc, of
//! [`foldICmpInstWithConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L3644),
//! [`foldICmpBinOpWithConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L4124),
//! [`foldICmpAndConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1951),
//! [`foldICmpAndConstConst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1807)
//! and
//! [`foldICmpAndShift`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1714).
//!
//! MIRROR: the order of the folds and each condition.
//!
//! OWED, each at its place as a `// OWED:` comment: the other opcodes of
//! `foldICmpBinOpWithConstant` (`foldICmpXorConstant`, `foldICmpOrConstant`,
//! `foldICmpMulConstant`, `foldICmpShlConstant`, `foldICmpShrConstant`,
//! `foldICmpSRemConstant`, `foldICmpUDivConstant`, `foldICmpDivConstant`,
//! `foldICmpSubConstant`, `foldICmpAddConstant`),
//! `foldICmpBinOpEqualityWithConstant`, the truth-table fold, and the select,
//! truncate, intrinsic and `extractvalue` arms of `foldICmpInstWithConstant`.
//!
//! DEVIATE 1: the load from an indexed global fold
//! (`foldCmpLoadFromIndexedGlobal`) and the floating-point class folds on a
//! bitcast (`is.fpclass`) are not translated.
//!
//! # How it works
//! The compare's left operand is an `and` of a value with a constant `C2`,
//! compared with a constant `C1`. The `and` goes where the compare says what
//! it keeps: `(X & signbit) != 0` is `X < 0`, `(X & -2^n) == 0` is `X u< 2^n`,
//! `(X & 1) != 0` is `trunc X`, a shift under the mask moves into the
//! constants, and a mask of a narrowed value widens.
//!
//! # Used by
//! `combine_compares::visit_icmp`.
//!
//! # Example
//! A sign test of a mask is a compare of the value:
//!
//! ```text
//! %m = and i16 %x, -32768
//! %r = icmp ne i16 %m, 0          ; %r = icmp slt i16 %x, 0
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::fp::is_sign_bit_check;
use llrm_mir::valuetracking::{Query, known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};

/// A new compare in place of `inst`: `new ICmpInst(pred, a, b)`.
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

/// `new TruncInst(x, i1)`.
fn new_trunc_to_bool(
    c: &mut Combiner,
    inst: InstId,
    x: Operand,
) -> Visit {
    let i1 = c.context.types.int(1);
    c.replace_with_new(inst, Opcode::Cast(CastOp::Trunc), i1, vec![x], Flags::default())
}

/// `APInt::isNegatedPowerOf2`.
fn is_negated_power_of_two(
    value: u128,
    width: u32,
) -> bool {
    value.wrapping_neg() & mask(width) != 0 && (value.wrapping_neg() & mask(width)).is_power_of_two()
}

fn is_negative(
    value: u128,
    width: u32,
) -> bool {
    value >> (width - 1) & 1 == 1
}

fn is_signed(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle)
}

fn is_equality(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
}

/// `m_c_And(m_Neg(X), m_Deferred(X))` of the operands of an `and`: `X`.
fn neg_and_self(
    q: &Query,
    a: Operand,
    b: Operand,
) -> Option<Operand> {
    let neg_of = |v: Operand| {
        let (_, _, zero, x) = pattern::binary(q, v, BinaryOp::Sub)?;
        (q.int_constant(zero)?.0 == 0).then_some(x)
    };
    [(a, b), (b, a)].into_iter().find_map(|(neg, other)| neg_of(neg).filter(|&x| x == other))
}

/// `foldICmpInstWithConstant`.
pub fn fold_icmp_inst_with_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (value, width) = query(c, None).int_constant(op1)?;
    let (and_x, and_y) = {
        let q = query(c, None);
        let (_, _, x, y) = pattern::binary(&q, op0, BinaryOp::And)?;
        (x, y)
    };
    // foldICmpBinOpWithConstant, the `and` case.
    // OWED: the other opcodes, foldICmpBinOpEqualityWithConstant and the
    // truth-table fold; the select, trunc, intrinsic and extractvalue arms.
    fold_icmp_and_constant(c, inst, pred, op0, (and_x, and_y), value, width)
}

/// `foldICmpAndConstant`: `icmp (and X, Y), C`.
fn fold_icmp_and_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    and: Operand,
    (x, y): (Operand, Operand),
    value: u128,
    width: u32,
) -> Option<Visit> {
    if let Some(found) = fold_icmp_and_const_const(c, inst, pred, and, (x, y), value, width) {
        return Some(found);
    }
    if let Some(true_if_neg) = is_sign_bit_check(pred, value, width) {
        let q = query(c, Some(inst));
        // ((X - 1) & ~X) <  0 --> X == 0
        // ((X - 1) & ~X) >= 0 --> X != 0
        let sub_one = pattern::binary(&q, x, BinaryOp::Add)
            .filter(|&(_, _, _, ones)| q.int_constant(ones).is_some_and(|(bits, w)| bits == mask(w)))
            .map(|(_, _, a, _)| a)
            .filter(|&a| pattern::not(&q, y) == Some(a));
        let min_signed_x = neg_and_self(&q, x, y);
        let ty = c.type_of(x);
        if let Some(a) = sub_one {
            let new_pred = if true_if_neg { IntPredicate::Eq } else { IntPredicate::Ne };
            let zero = c.int(ty, 0);
            return Some(new_icmp(c, inst, new_pred, a, zero));
        }
        // (X & -X) <  0 --> X == MinSignedC
        // (X & -X) > -1 --> X != MinSignedC
        if let Some(a) = min_signed_x {
            let new_pred = if true_if_neg { IntPredicate::Eq } else { IntPredicate::Ne };
            let min_signed = c.int(ty, 1u128 << (width - 1));
            return Some(new_icmp(c, inst, new_pred, a, min_signed));
        }
    }
    // OWED: foldCmpLoadFromIndexedGlobal (DEVIATE 1).
    if !is_equality(pred) {
        return None;
    }
    let one_use_and = has_one_use(c, and);
    let ty = c.type_of(and);
    // (X & -X) == 0 --> X == 0
    // (X & -X) != 0 --> X != 0
    // (X & -X) == 1 --> trunc X to i1
    // (X & -X) != 1 --> !(trunc X to i1)
    if width > 1 && (value == 0 || value == 1) {
        let matched = neg_and_self(&query(c, Some(inst)), x, y);
        if let Some(m) = matched {
            if value == 0 {
                let zero = c.int(ty, 0);
                return Some(new_icmp(c, inst, pred, m, zero));
            }
            if pred == IntPredicate::Eq {
                return Some(new_trunc_to_bool(c, inst, m));
            }
            // The original `and` must become dead.
            if one_use_and {
                let i1 = c.context.types.int(1);
                let trunc = c.create_cast(inst, CastOp::Trunc, m, i1, Flags::default());
                return Some(Visit::Replace(c.create_not(inst, trunc)));
            }
        }
    }
    // X & -C == -C -> X >  u ~C
    // X & -C != -C -> X <= u ~C
    //   iff C is a power of 2
    let cmp_rhs = c.function.instruction(inst).operands[1];
    if cmp_rhs == y && is_negated_power_of_two(value, width) {
        let new_pred = if pred == IntPredicate::Eq { IntPredicate::Ugt } else { IntPredicate::Ule };
        let minus_one = c.int(ty, value.wrapping_sub(1) & mask(width));
        return Some(new_icmp(c, inst, new_pred, x, minus_one));
    }
    // ((zext i1 X) & Y) == 0 --> !((trunc Y) & X)
    // ((zext i1 X) & Y) != 0 -->  ((trunc Y) & X)
    // ((zext i1 X) & Y) == 1 -->  ((trunc Y) & X)
    // ((zext i1 X) & Y) != 1 --> !((trunc Y) & X)
    if one_use_and && (value == 0 || value == 1) {
        let found = [(x, y), (y, x)].into_iter().find_map(|(first, other)| {
            let q = query(c, Some(inst));
            let (_, _, bit) = pattern::cast(&q, first, CastOp::ZExt)?;
            (has_one_use(c, first) && q.scalar_bits(bit) == 1).then_some((bit, other))
        });
        if let Some((bit, other)) = found {
            let i1 = c.context.types.int(1);
            let trunc_y = c.create_cast(inst, CastOp::Trunc, other, i1, Flags::default());
            if (value == 0) ^ (pred == IntPredicate::Ne) {
                let both = c.create_binop(inst, BinaryOp::And, trunc_y, bit, Flags::default());
                return Some(Visit::Replace(c.create_not(inst, both)));
            }
            return Some(c.replace_with_binop(inst, BinaryOp::And, trunc_y, bit, Flags::default()));
        }
    }
    // (icmp eq/ne (and (shl -1, X), Y), 0)
    //    -> (icmp eq/ne (lshr Y, X), 0)
    if value == 0 && one_use_and {
        let found = [(x, y), (y, x)].into_iter().find_map(|(first, other)| {
            let q = query(c, Some(inst));
            let (_, _, ones, amount) = pattern::binary(&q, first, BinaryOp::Shl)?;
            let all_ones = q.int_constant(ones).is_some_and(|(bits, w)| bits == mask(w));
            (has_one_use(c, first) && all_ones).then_some((amount, other))
        });
        if let Some((amount, other)) = found {
            let lshr = c.create_binop(inst, BinaryOp::LShr, other, amount, Flags::default());
            let zero = c.int(c.type_of(lshr), 0);
            return Some(new_icmp(c, inst, pred, lshr, zero));
        }
    }
    // (icmp eq/ne (and (add A, Addend), Msk), C)
    //    -> (icmp eq/ne (and A, Msk), (and (sub C, Addend), Msk))
    if one_use_and {
        let found = {
            let q = query(c, Some(inst));
            (|| {
                let (a, addend) = {
                    let (_, _, a, addend) = pattern::binary(&q, x, BinaryOp::Add)?;
                    (a, q.int_constant(addend)?.0)
                };
                let (msk, _) = pattern::low_bit_mask(&q, y)?;
                (has_one_use(c, x) && value <= msk).then_some((a, addend, msk))
            })()
        };
        if let Some((a, addend, msk)) = found {
            let new_comperand = value.wrapping_sub(addend) & msk;
            let mask_value = c.int(c.type_of(a), msk);
            let mask_a = c.create_binop(inst, BinaryOp::And, a, mask_value, Flags::default());
            let comperand = c.int(c.type_of(mask_a), new_comperand);
            return Some(new_icmp(c, inst, pred, mask_a, comperand));
        }
    }
    None
}

/// `foldICmpAndConstConst`: `icmp (and X, C2), C1`.
fn fold_icmp_and_const_const(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    and: Operand,
    (x, y): (Operand, Operand),
    c1: u128,
    width: u32,
) -> Option<Visit> {
    let is_ne = pred == IntPredicate::Ne;
    // icmp ne (and X, 1), 0 --> trunc X to i1
    if is_ne && c1 == 0 && query(c, None).int_constant(y) == Some((1, width)) {
        return Some(new_trunc_to_bool(c, inst, x));
    }
    let c2 = query(c, None).int_constant(y)?.0;
    let all = mask(width);
    // (and X, highmask) s> [0, ~highmask] --> X s> ~highmask
    if pred == IntPredicate::Sgt && c1 <= !c2 & all && is_negated_power_of_two(c2, width) {
        let bound = c.int(c.type_of(x), !c2 & all);
        return Some(new_icmp(c, inst, IntPredicate::Sgt, x, bound));
    }
    // (and X, highmask) s< [1, -highmask] --> X s< -highmask
    let sign = 1u128 << (width - 1);
    if pred == IntPredicate::Slt
        && c1 != sign
        && c1.wrapping_sub(1) & all <= !c2 & all
        && is_negated_power_of_two(c2, width)
        && c2 != sign
    {
        let bound = c.int(c.type_of(x), c2.wrapping_neg() & all);
        return Some(new_icmp(c, inst, IntPredicate::Slt, x, bound));
    }
    // Don't perform the following transforms if the AND has multiple uses.
    if !has_one_use(c, and) {
        return None;
    }
    if is_equality(pred) && c1 == 0 {
        // Replace (and X, (1 << size(X)-1) != 0) with X s< 0
        if c2 == sign {
            let zero = c.int(c.type_of(x), 0);
            let new_pred = if is_ne { IntPredicate::Slt } else { IntPredicate::Sge };
            return Some(new_icmp(c, inst, new_pred, x, zero));
        }
        let and_inst = query(c, None).instruction_of(and)?.0;
        let know = {
            let q = query(c, Some(and_inst));
            known::compute_known_bits(&q, x, 0)
        };
        // Set high zeros of C2 to allow matching negated power-of-2.
        let lead = know.count_min_leading_zeros().min(width);
        let high = if lead == 0 { 0 } else { all & !(all >> lead) };
        let new_c2 = c2 | high;
        // ((%x & C) == 0) --> %x u< (-C)  iff (-C) is power of two.
        if is_negated_power_of_two(new_c2, width) {
            let neg_c = c.int(c.type_of(and), new_c2.wrapping_neg() & all);
            let new_pred = if is_ne { IntPredicate::Uge } else { IntPredicate::Ult };
            return Some(new_icmp(c, inst, new_pred, x, neg_c));
        }
    }
    // icmp (and (trunc W), C2), C1 -> icmp (and W, C2'), C1', where the
    // constants have no sign bit or the compare is an equality.
    let wide = {
        let q = query(c, Some(inst));
        pattern::cast(&q, x, CastOp::Trunc).map(|(_, _, w)| w).filter(|_| has_one_use(c, x))
    };
    if let Some(w) = wide
        && (is_equality(pred) || (!is_negative(c1, width) && !is_negative(c2, width)))
    {
        let wide_ty = c.type_of(w);
        let zext_c1 = c.int(wide_ty, c1);
        let zext_c2 = c.int(wide_ty, c2);
        let new_and = c.create_binop(inst, BinaryOp::And, w, zext_c2, Flags::default());
        return Some(new_icmp(c, inst, pred, new_and, zext_c1));
    }
    if let Some(found) = fold_icmp_and_shift(c, inst, pred, and, (x, y), c1, c2, width) {
        return Some(found);
    }
    // (icmp pred (and (or (lshr A, B), A), 1), 0) -->
    // (icmp pred (and A, (or (shl 1, B), 1), 0))
    // iff pred isn't signed
    if !is_signed(pred) && c1 == 0 && has_one_use(c, x) && query(c, None).int_constant(y) == Some((1, width)) {
        let found = {
            let q = query(c, Some(inst));
            (|| {
                let (_, _, lshr, a) = pattern::binary(&q, x, BinaryOp::Or)?;
                let (_, _, shifted, b) = pattern::binary(&q, lshr, BinaryOp::LShr)?;
                (shifted == a).then_some((lshr, a, b))
            })()
        };
        if let Some((lshr, a, b)) = found {
            let removed =
                u32::from(has_one_use(c, and)) + u32::from(has_one_use(c, x)) + u32::from(has_one_use(c, lshr));
            // Compute A & ((1 << B) | 1)
            let imm = matches!(b, Operand::Constant(_));
            let required = if imm { 1 } else { 3 };
            if removed >= required {
                let one = y;
                let shl = c.create_binop(inst, BinaryOp::Shl, one, b, Flags::NUW);
                let new_or = c.create_binop(inst, BinaryOp::Or, shl, one, Flags::default());
                let new_and = c.create_binop(inst, BinaryOp::And, a, new_or, Flags::default());
                let rhs = c.function.instruction(inst).operands[1];
                return Some(new_icmp(c, inst, pred, new_and, rhs));
            }
        }
    }
    // OWED: the `is.fpclass` folds of an `and` of a bitcast (DEVIATE 1).
    None
}

/// `foldICmpAndShift`: `icmp (and (sh X, Y), C2), C1`.
fn fold_icmp_and_shift(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    and: Operand,
    (shift, and_y): (Operand, Operand),
    c1: u128,
    c2: u128,
    width: u32,
) -> Option<Visit> {
    let (shift_op, shift_x, shift_y) = {
        let q = query(c, Some(inst));
        let (_, instruction) = q.instruction_of(shift)?;
        let Opcode::Binary(op @ (BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)) = instruction.opcode else {
            return None;
        };
        (op, instruction.operands[0], instruction.operands[1])
    };
    let all = mask(width);
    let is_shl = shift_op == BinaryOp::Shl;
    let c3 = query(c, None).int_constant(shift_y).map(|(bits, _)| bits);
    // If this is: (X >> C3) & C2 != C1 (where any shift and any compare could
    // exist), turn it into (X & (C2 << C3)) != (C1 << C3).
    if let Some(c3) = c3 {
        let sh = c3.min(u128::from(u32::MAX)) as u32;
        let wide = |v: u128| if sh >= width { 0 } else { v };
        let (new_and, new_cmp, shifted_out);
        let ashr = |v: u128| {
            let negative = is_negative(v, width);
            let s = sh.min(width - 1);
            let shifted = v >> s;
            if negative { (shifted | (all & !(all >> s))) & all } else { shifted }
        };
        match shift_op {
            BinaryOp::Shl => {
                // For a left shift, we can fold if the comparison is not
                // signed, or the mask and comparison values are not negative.
                if is_signed(pred) && (is_negative(c2, width) || is_negative(c1, width)) {
                    return None;
                }
                new_cmp = wide(c1 >> sh.min(127));
                new_and = wide(c2 >> sh.min(127));
                shifted_out = wide((new_cmp << sh.min(127)) & all) != c1;
            }
            BinaryOp::LShr => {
                new_cmp = wide((c1 << sh.min(127)) & all);
                new_and = wide((c2 << sh.min(127)) & all);
                shifted_out = wide(new_cmp >> sh.min(127)) != c1;
                if is_signed(pred) && (is_negative(new_and, width) || is_negative(new_cmp, width)) {
                    return None;
                }
            }
            _ => {
                new_cmp = wide((c1 << sh.min(127)) & all);
                new_and = wide((c2 << sh.min(127)) & all);
                shifted_out = ashr(new_cmp) != c1;
                if ashr(new_and) != c2 {
                    return None;
                }
            }
        }
        if shifted_out {
            // As a special case, the result is always true or false now.
            let i1 = c.context.types.int(1);
            if pred == IntPredicate::Eq {
                return Some(Visit::Replace(c.int(i1, 0)));
            }
            if pred == IntPredicate::Ne {
                return Some(Visit::Replace(c.int(i1, 1)));
            }
        } else {
            let ty = c.type_of(and);
            let and_const = c.int(ty, new_and);
            let masked = c.create_binop(inst, BinaryOp::And, shift_x, and_const, Flags::default());
            let cmp_const = c.int(ty, new_cmp);
            return Some(new_icmp(c, inst, pred, masked, cmp_const));
        }
    }
    // Turn ((X >> Y) & C2) == 0  into  (X & (C2 << Y)) == 0.
    let not_const_x = !matches!(shift_x, Operand::Constant(_));
    if has_one_use(c, shift)
        && c1 == 0
        && is_equality(pred)
        && shift_op != BinaryOp::AShr
        && ((!is_shl && c2 == 1) || not_const_x)
    {
        // Compute C2 << Y.
        let new_shift = if is_shl {
            c.create_binop(inst, BinaryOp::LShr, and_y, shift_y, Flags::default())
        } else {
            c.create_binop(inst, BinaryOp::Shl, and_y, shift_y, Flags::default())
        };
        // Compute X & (C2 << Y).
        let new_and = c.create_binop(inst, BinaryOp::And, shift_x, new_shift, Flags::default());
        let rhs = c.function.instruction(inst).operands[1];
        return Some(new_icmp(c, inst, pred, new_and, rhs));
    }
    None
}

#[cfg(test)]
#[path = "combine_icmp_const_tests.rs"]
mod tests;
