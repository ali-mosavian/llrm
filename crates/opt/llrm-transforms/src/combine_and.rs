//! `InstCombinerImpl::visitAnd`: the integer `and` folds.
//!
//! Translation of
//! [`visitAnd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2475)
//! and
//! [`foldAndToXor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L1944)
//! in the order `visitAnd` reaches them, up to the constant-mask folds.
//!
//! MIRROR: the C++ order, including branches a later fold shadows.
//!
//! DEVIATE 1: no vectors (`foldVectorBinop`).
//!
//! DEVIATE 2: `foldComplexAndOrPatterns` is the next strip or another family's.
//!
//! DEVIATE 3: of the shifted-power-of-two folds (:2688-:2740) only the
//! single-shift one is translated; the two double-shift ones follow.
//!
//! DEVIATE 4: from the `and` of `icmp`s (`foldAndOrOfICmps`) on, the rest of
//! `visitAnd` is the next strip.
//!
//! # How it works
//! The prologue (simplify, associativity, phi operands, `foldAndToXor`) runs
//! first; then folds of an `and` with a constant mask, each rebuilding the
//! operands with the `Combiner` builder.
//!
//! # Used by
//! `combine::visit` (the `And` arm, once wired).
//!
//! # Example
//! `(x ^ 12) & 10` is `(x & 10) ^ 8`:
//!
//! ```text
//! %e = xor i32 %x, 12
//! %r = and i32 %e, 10   ; %a = and i32 %x, 10 ; %r = xor i32 %a, 8
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::power2::is_known_to_be_a_power_of_two;
use llrm_mir::valuetracking::{Query, known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_and_select::fold_and_select;
use crate::combine_bitorder::fold_bitwise_logic_with_intrinsics;
use crate::combine_distribute::fold_using_distributive_laws;
use crate::combine_icmp_logic::{fold_boolean_and_or, reassociate_boolean_operands};
use crate::combine_shared::{
    binop, commutative, has_one_use, inst_def, parts, query, simplify_associative_or_commutative, simplify_visited,
    width,
};
use crate::combine_shift_binop::fold_binop_shift_with_shift;

/// `m_Not(m_Value())` of `v`.
fn not_of(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    pattern::not(q, v)
}

/// `m_c_<op>(m_Specific(a), m_Specific(b))`: `v` is `op a, b` or `op b, a`.
fn is_pair(
    q: &Query,
    v: Operand,
    op: BinaryOp,
    a: Operand,
    b: Operand,
) -> bool {
    binop(q, v, op).is_some_and(|(x, y, _)| (x, y) == (a, b) || (x, y) == (b, a))
}

/// `m_c_Or(m_Value(A), m_Not(m_Value(B)))`: every `(A, B)` the commutative
/// match can bind, in the order it tries them.
fn or_with_not(
    q: &Query,
    v: Operand,
) -> Vec<(Operand, Operand)> {
    let Some((x, y, _)) = binop(q, v, BinaryOp::Or) else { return Vec::new() };
    [(x, y), (y, x)].into_iter().filter_map(|(a, other)| not_of(q, other).map(|b| (a, b))).collect()
}

/// `foldAndToXor` (:1944).
fn fold_and_to_xor(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let found = {
        let q = query(c, Some(inst));
        // (A | B) & ~(A & B) --> A ^ B
        let xor = binop(&q, op0, BinaryOp::Or).and_then(|(a, b, _)| {
            let inner = not_of(&q, op1)?;
            is_pair(&q, inner, BinaryOp::And, a, b).then_some((a, b))
        });
        if xor.is_some() {
            xor.map(|(a, b)| (a, b, false))
        } else if has_one_use(c, op0) || has_one_use(c, op1) {
            // (A | ~B) & (~A | B) --> ~(A ^ B)
            or_with_not(&q, op0)
                .into_iter()
                .find_map(
                    |(a, b)| {
                        let matches = binop(&q, op1, BinaryOp::Or).is_some_and(|(x, y, _)| {
                            (not_of(&q, x) == Some(a) && y == b) || (not_of(&q, y) == Some(a) && x == b)
                        });
                        matches.then_some((a, b, true))
                    },
                )
        } else {
            None
        }
    };
    let (a, b, inverted) = found?;
    if inverted {
        let xor = c.create_binop(inst, BinaryOp::Xor, a, b, Flags::default());
        let ones = c.all_ones(c.type_of(xor));
        return Some(c.replace_with_binop(inst, BinaryOp::Xor, xor, ones, Flags::default()));
    }
    Some(c.replace_with_binop(inst, BinaryOp::Xor, a, b, Flags::default()))
}

/// `(1 >> X) & 1` and `(C << X) & 1` (C odd) are `zext(X == 0)` (:2514).
fn fold_one_shifted_by(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let x = {
        let q = query(c, Some(inst));
        if !has_one_use(c, op0) || !pattern::one(&q, op1) {
            return None;
        }
        let logical = [BinaryOp::LShr, BinaryOp::Shl]
            .into_iter()
            .find_map(|op| binop(&q, op0, op).filter(|&(base, ..)| pattern::one(&q, base)).map(|(_, x, _)| x));
        logical.or_else(|| {
            let (base, x, _) = binop(&q, op0, BinaryOp::Shl)?;
            pattern::apint(&q, base).filter(|&(bits, _)| bits & 1 == 1).map(|_| x)
        })?
    };
    let ty = c.type_of(op0);
    let zero = c.int(c.type_of(x), 0);
    let is_zero = c.create_icmp(inst, IntPredicate::Eq, x, zero);
    Some(c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), ty, vec![is_zero], Flags::default()))
}

/// `(-(X & 1)) & Y` is `(X & 1) == 0 ? 0 : Y` (:2526).
fn fold_negated_low_bit(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (neg, y) = {
        let q = query(c, Some(inst));
        let is_neg_of_low_bit = |v: Operand| {
            has_one_use(c, v)
                && pattern::neg(&q, v)
                    .and_then(|(_, inner, _)| binop(&q, inner, BinaryOp::And))
                    .is_some_and(|(_, one, _)| pattern::one(&q, one))
        };
        if is_neg_of_low_bit(op0) {
            (op0, op1)
        } else if is_neg_of_low_bit(op1) {
            (op1, op0)
        } else {
            return None;
        }
    };
    let ty = c.type_of(neg);
    let zero = c.int(ty, 0);
    let cmp = c.create_icmp(inst, IntPredicate::Eq, neg, zero);
    Some(c.replace_with_new(inst, Opcode::Select, ty, vec![cmp, zero, y], Flags::default()))
}

/// `(X +/- Y) & Y` is `~X & Y` when `Y` is a power of two (:2537).
fn fold_power_of_two_sum(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let found = {
        let q = query(c, Some(inst));
        let shape = |y: Operand, sum: Operand| {
            if !has_one_use(c, sum) {
                return None;
            }
            let x = commutative(&q, sum, BinaryOp::Add, |s| s == y)
                .map(|(_, x)| x)
                .or_else(|| binop(&q, sum, BinaryOp::Sub).filter(|&(_, s, _)| s == y).map(|(x, ..)| x))?;
            is_known_to_be_a_power_of_two(&q, y, true, 0).then_some((x, y))
        };
        shape(op0, op1).or_else(|| shape(op1, op0))
    };
    let (x, y) = found?;
    let not = c.create_not(inst, x);
    Some(c.replace_with_binop(inst, BinaryOp::And, not, y, Flags::default()))
}

/// The folds of `and X, C`: `op1` is an integer constant (:2549).
fn fold_and_with_constant(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (k, w) = pattern::apint(&query(c, Some(inst)), op1)?;
    let ty = c.type_of(op0);
    let one_use = has_one_use(c, op0);
    let inner = {
        let q = query(c, Some(inst));
        let xor = binop(&q, op0, BinaryOp::Xor).and_then(|(x, m, _)| pattern::apint(&q, m).map(|(m, _)| (x, m)));
        let or = binop(&q, op0, BinaryOp::Or).and_then(|(x, m, _)| pattern::apint(&q, m).map(|(m, _)| (x, m)));
        let ashr = binop(&q, op0, BinaryOp::AShr).and_then(|(x, m, _)| pattern::apint(&q, m).map(|(m, _)| (x, m)));
        let sext_ashr = pattern::cast(&q, op0, CastOp::SExt).and_then(|(_, _, a)| {
            binop(&q, a, BinaryOp::AShr).and_then(|(x, m, _)| pattern::apint(&q, m).map(|(m, _)| (x, m)))
        });
        let add = binop(&q, op0, BinaryOp::Add).and_then(|(x, m, _)| pattern::apint(&q, m).map(|(m, _)| (x, m)));
        (xor, or, ashr, sext_ashr, add)
    };
    let (xor, or, ashr, sext_ashr, add) = inner;
    // (X ^ C1) & C2 --> (X & C2) ^ (C1&C2)
    if one_use && let Some((x, xor_c)) = xor {
        let new_c = c.int(ty, k & xor_c);
        let and = c.create_binop(inst, BinaryOp::And, x, op1, Flags::default());
        return Some(c.replace_with_binop(inst, BinaryOp::Xor, and, new_c, Flags::default()));
    }
    // (X | C1) & C2 --> (X & C2^(C1&C2)) | (C1&C2)
    if one_use && let Some((x, or_c)) = or {
        let together = k & or_c;
        let mask_c = c.int(ty, together ^ k);
        let and = c.create_binop(inst, BinaryOp::And, x, mask_c, Flags::default());
        let together_c = c.int(ty, together);
        return Some(c.replace_with_binop(inst, BinaryOp::Or, and, together_c, Flags::default()));
    }
    // and (sext (ashr X, ShiftC)), C --> lshr (sext X), ShiftC
    if one_use
        && let Some((x, shift)) = sext_ashr
        && shift < w as u128
        && k == mask(w) >> shift
    {
        let sext = c.create_cast(inst, CastOp::SExt, x, ty, Flags::default());
        let amount = c.int(ty, shift);
        return Some(c.replace_with_binop(inst, BinaryOp::LShr, sext, amount, Flags::default()));
    }
    // and (ashr X, ShiftC), C --> lshr X, ShiftC when C clears the smeared bits
    if let Some((x, shift)) = ashr
        && shift < w as u128
        && k == mask(w) >> shift
    {
        let amount = c.int(ty, shift);
        return Some(c.replace_with_binop(inst, BinaryOp::LShr, x, amount, Flags::default()));
    }
    // (X + 4) & 4 --> (X & 4) ^ 4
    if let Some((x, add_c)) = add
        && one_use
        && k.is_power_of_two()
        && add_c & (k - 1) == 0
    {
        let new_and = c.create_binop(inst, BinaryOp::And, x, op1, Flags::default());
        return Some(c.replace_with_binop(inst, BinaryOp::Xor, new_and, op1, Flags::default()));
    }
    None
}

/// `and (bo (zext X), C1), C` and the masks of a `zext` operand: the `and` and
/// the binop narrowed to `X`'s type (:2595).
fn fold_narrowed_binop(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (k, _) = pattern::apint(&query(c, Some(inst)), op1)?;
    if !has_one_use(c, op0) {
        return None;
    }
    let ty = c.type_of(op0);
    let (op, a, b) = {
        let instruction = c.function.instruction(inst_def(c, op0)?);
        let Opcode::Binary(op) = instruction.opcode else { return None };
        if !matches!(
            op,
            BinaryOp::Xor | BinaryOp::Or | BinaryOp::Mul | BinaryOp::Add | BinaryOp::Sub
        ) {
            return None;
        }
        (op, instruction.operands[0], instruction.operands[1])
    };
    // and (bo (zext X), C1), C --> zext (and (bo X, TruncC1), TruncC)
    let zext_const = {
        let q = query(c, Some(inst));
        let zext = |v: Operand| {
            if !has_one_use(c, v) {
                return None;
            }
            pattern::cast(&q, v, CastOp::ZExt).map(|(_, _, x)| x)
        };
        zext(a)
            .zip(pattern::apint(&q, b))
            .map(|(x, (c1, _))| (x, c1, true))
            .or_else(|| zext(b).zip(pattern::apint(&q, a)).map(|(x, (c1, _))| (x, c1, false)))
    };
    if let Some((x, c1, zext_first)) = zext_const {
        let x_ty = c.type_of(x);
        let xw = width(c, x);
        if k <= mask(xw) {
            let trunc_c1 = c.int(x_ty, c1 & mask(xw));
            let (l, r) = if zext_first { (x, trunc_c1) } else { (trunc_c1, x) };
            let bin = c.create_binop(inst, op, l, r, Flags::default());
            let trunc_c = c.int(x_ty, k & mask(xw));
            let and = c.create_binop(inst, BinaryOp::And, bin, trunc_c, Flags::default());
            return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), ty, vec![and], Flags::default()));
        }
    }
    // and (bo (zext X), Y), C --> zext (bo X, (trunc Y)), and the mirror
    for (zext_side, other, zext_first) in [(a, b, true), (b, a, false)] {
        let x = {
            let q = query(c, Some(inst));
            let defined = inst_def(c, zext_side).is_some();
            if !defined || !has_one_use(c, zext_side) {
                continue;
            }
            match pattern::cast(&q, zext_side, CastOp::ZExt) {
                Some((_, _, x)) if k == mask(width(c, x)) => x,
                _ => continue,
            }
        };
        let x_ty = c.type_of(x);
        let tr = c.create_cast(inst, CastOp::Trunc, other, x_ty, Flags::default());
        let (l, r) = if zext_first { (x, tr) } else { (tr, x) };
        let narrow = c.create_binop(inst, op, l, r, Flags::default());
        return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), ty, vec![narrow], Flags::default()));
    }
    None
}

/// `and ({x}or X, Y), C`: the mask moves to the arm that needs it (:2650).
fn fold_mask_into_one_arm(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (k, w) = pattern::apint(&query(c, Some(inst)), op1)?;
    if !has_one_use(c, op0) {
        return None;
    }
    let (op, x, y) = {
        let q = query(c, Some(inst));
        [BinaryOp::Xor, BinaryOp::Or].into_iter().find_map(|op| binop(&q, op0, op).map(|(x, y, _)| (op, x, y)))?
    };
    let not_and_mask = !k & mask(w);
    let zero_in = |c: &Combiner, v: Operand| known::masked_value_is_zero(&query(c, Some(inst)), v, not_and_mask, 0);
    // Not masking anything out for the LHS, move the mask to the RHS.
    if zero_in(c, x) {
        let new_rhs = c.create_binop(inst, BinaryOp::And, y, op1, Flags::default());
        return Some(c.replace_with_binop(inst, op, x, new_rhs, Flags::default()));
    }
    // Not masking anything out for the RHS, move the mask to the LHS.
    if !matches!(y, Operand::Constant(_)) && zero_in(c, y) {
        let new_lhs = c.create_binop(inst, BinaryOp::And, x, op1, Flags::default());
        return Some(c.replace_with_binop(inst, op, new_lhs, y, Flags::default()));
    }
    None
}

/// `(ShiftC << X) & C` is `X == bit ? C : 0` for powers of two (:2680).
fn fold_shifted_power_of_two(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (k, _) = pattern::apint(&query(c, Some(inst)), op1)?;
    if !k.is_power_of_two() || !has_one_use(c, op0) {
        return None;
    }
    let (shift_c, x, left) = {
        let q = query(c, Some(inst));
        [BinaryOp::Shl, BinaryOp::LShr]
            .into_iter()
            .find_map(
                |op| {
                    let (base, x, _) = binop(&q, op0, op)?;
                    pattern::power2_constant(&q, base).map(|base| (base, x, op == BinaryOp::Shl))
                },
            )?
    };
    let (log_shift, log_c) = (shift_c.trailing_zeros() as i64, k.trailing_zeros() as i64);
    let bit = if left { log_c - log_shift } else { log_shift - log_c };
    debug_assert!(bit >= 0, "the demanded bits drop an impossible mask");
    let ty = c.type_of(op0);
    let at = c.int(c.type_of(x), bit.max(0) as u128);
    let cmp = c.create_icmp(inst, IntPredicate::Eq, x, at);
    let zero = c.int(ty, 0);
    Some(c.replace_with_new(inst, Opcode::Select, ty, vec![cmp, op1, zero], Flags::default()))
}

/// `InstCombinerImpl::visitAnd` (:2475).
pub fn visit_and(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // simplifyAndInst
    if let Some(found) = simplify_visited(c, inst) {
        return found;
    }
    if simplify_associative_or_commutative(c, inst) {
        return Visit::Changed;
    }
    // DEVIATE 1: foldVectorBinop.
    let found = c.fold_binop_with_phi_operands(inst);
    if !matches!(found, Visit::None) {
        return found;
    }
    // See if we can simplify any instructions used by the instruction whose
    // sole purpose is to compute bits we don't care about.
    let found = c.simplify_demanded_instruction_bits(inst);
    if !matches!(found, Visit::None) {
        return found;
    }
    let (op0, op1, _) = parts(c, inst);
    // Do this before using distributive laws to catch simple and/or/not
    // patterns.
    if let Some(found) = fold_and_to_xor(c, inst, op0, op1) {
        return found;
    }
    // DEVIATE 2: foldComplexAndOrPatterns.
    if let Some(v) = fold_using_distributive_laws(c, inst) {
        return Visit::Replace(v);
    }
    if let Some(r) = fold_binop_shift_with_shift(c, inst) {
        return r;
    }
    let steps: [fn(&mut Combiner, InstId, Operand, Operand) -> Option<Visit>; 8] = [
        fold_one_shifted_by,
        fold_negated_low_bit,
        fold_power_of_two_sum,
        fold_and_with_constant,
        fold_narrowed_binop,
        fold_mask_into_one_arm,
        fold_shifted_power_of_two,
        fold_bitwise_logic_with_intrinsics,
    ];
    // DEVIATE 3 and 4: the rest of visitAnd.
    if let Some(found) = steps.iter().find_map(|step| step(c, inst, op0, op1)) {
        return found;
    }
    if let Some(found) = fold_and_select(c, inst, op0, op1) {
        return found;
    }
    // DEVIATE 4: the rest of visitAnd after this.
    if let Some(found) = fold_boolean_and_or(c, inst, op0, op1, true, false) {
        return Visit::Replace(found);
    }
    reassociate_boolean_operands(c, inst, (op0, op1), true).map_or(Visit::None, Visit::Replace)
}

#[cfg(test)]
#[path = "combine_and_tests.rs"]
mod tests;
