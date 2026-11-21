//! `foldXorOfICmps`: an `xor` of two `i1` compares, folded to one compare.
//!
//! Translation of
//! [`foldXorOfICmps`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L4864)
//! at d1106deb71cc.
//!
//! # How it works
//! In order: two compares of the same operands merge by their compare codes;
//! two sign-bit tests are one sign-bit test of the `xor` of the values; two
//! compares of one value with constants are one range test where the range
//! type holds `(a | b) & !(a & b)` exactly; two single-bit tests under one
//! power of two are one test of the `xor`. Last, when `or` and `and` of the
//! pair fold (`simplifyBinOp`) to one side and the other, `a ^ b` is
//! `a & !b`, which inverts `b`'s predicate in place.
//!
//! # Example
//! Whether two values differ in sign:
//!
//! ```text
//! %a = icmp sgt i32 %x, -1                    %v = xor i32 %x, %y
//! %b = icmp sgt i32 %y, -1          ==>       %r = icmp slt i32 %v, 0
//! %r = xor i1 %a, %b
//! ```
//!
//! MIRROR: the C++ order and the one-use conditions.
//!
//! DEVIATE 1: scalars only; the pin matches splat vector constants
//! (`m_APInt`). Exit: the vector `ConstantRange`.
//!
//! # Used by
//! `visit_xor`.

use llrm_mir::constantrange::Range;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::simplify::simplify_binop_at;
use llrm_mir::valuetracking::fp::is_sign_bit_check;
use llrm_mir::valuetracking::pattern;
use llrm_mir::valuetracking::power2::is_known_to_be_a_power_of_two;

use crate::combine::{Combiner, Visit};
use crate::combine_icmp_logic::{icmp_code, is_equality, is_signed, new_icmp_value, predicates_foldable};
use crate::combine_shared::{has_one_use, query};

/// `m_And(m_Value(X), m_Value(P))` of `lhs` and `m_And(m_Value(Y),
/// m_Specific(P))` of `rhs`: `(X, Y, P)`.
fn and_pair_sharing_operand(
    c: &Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
) -> Option<(Operand, Operand, Operand)> {
    let q = query(c, Some(inst));
    let (_, _, x, pow2) = pattern::binary(&q, lhs, BinaryOp::And)?;
    let (_, _, a, b) = pattern::binary(&q, rhs, BinaryOp::And)?;
    let y = if b == pow2 {
        a
    } else if a == pow2 {
        b
    } else {
        return None;
    };
    Some((x, y, pow2))
}

/// The part of `foldXorOfICmps` that needs constants on both right sides.
fn fold_with_constants(
    c: &mut Combiner,
    inst: InstId,
    (lhs, rhs): (Operand, Operand),
    (pred_l, lhs0, lc): (IntPredicate, Operand, u128),
    (pred_r, rhs0, rc): (IntPredicate, Operand, u128),
    width: u32,
) -> Option<Operand> {
    let (lhs_one_use, rhs_one_use) = (has_one_use(c, lhs), has_one_use(c, rhs));

    // (X > -1) ^ (Y > -1) --> (X ^ Y) < 0, and the mixed forms.
    if let (Some(signed_l), Some(signed_r)) =
        (is_sign_bit_check(pred_l, lc, width), is_sign_bit_check(pred_r, rc, width))
        && (lhs_one_use || rhs_one_use)
    {
        let xor = c.create_binop(inst, BinaryOp::Xor, lhs0, rhs0, Flags::default());
        return Some(if signed_l == signed_r {
            let zero = c.int(c.type_of(xor), 0);
            c.create_icmp(inst, IntPredicate::Slt, xor, zero)
        } else {
            let ones = c.all_ones(c.type_of(xor));
            c.create_icmp(inst, IntPredicate::Sgt, xor, ones)
        });
    }

    // (icmp pred1 X, C1) ^ (icmp pred2 X, C2) as one range test.
    if lhs0 == rhs0 {
        let cr1 = Range::make_exact_icmp_region(pred_l, width, lc);
        let cr2 = Range::make_exact_icmp_region(pred_r, width, rc);
        if let (Some(union), Some(intersect)) = (cr1.exact_union_with(&cr2), cr1.exact_intersect_with(&cr2))
            && let Some(cr) = union.exact_intersect_with(&intersect.inverse())
        {
            let i1 = c.context.types.int(1);
            if cr.is_full_set() {
                return Some(c.int(i1, 1));
            }
            if cr.is_empty_set() {
                return Some(c.int(i1, 0));
            }
            let (pred, new_c, offset) = cr.get_equivalent_icmp_offset();
            if (offset == 0 && (lhs_one_use || rhs_one_use)) || (lhs_one_use && rhs_one_use) {
                let ty = c.type_of(lhs0);
                let mut new_v = lhs0;
                if offset != 0 {
                    let offset = c.int(ty, offset);
                    new_v = c.create_add(inst, new_v, offset);
                }
                let new_c = c.int(ty, new_c);
                return Some(c.create_icmp(inst, pred, new_v, new_c));
            }
        }
    }

    // (icmp eq/ne (X & Pow2), 0) ^ (icmp eq/ne (Y & Pow2), 0)
    //   --> (icmp eq/ne ((X ^ Y) & Pow2), 0)
    if is_equality(pred_l) && is_equality(pred_r) && lc == 0 && rc == 0 && lhs_one_use && rhs_one_use {
        let (x, y, pow2) = and_pair_sharing_operand(c, inst, lhs0, rhs0)?;
        let q = query(c, Some(inst));
        if is_known_to_be_a_power_of_two(&q, pow2, true, 0) {
            let xor = c.create_binop(inst, BinaryOp::Xor, x, y, Flags::default());
            let and = c.create_binop(inst, BinaryOp::And, xor, pow2, Flags::default());
            let zero = c.int(c.type_of(and), 0);
            let pred = if pred_l == pred_r { IntPredicate::Ne } else { IntPredicate::Eq };
            return Some(c.create_icmp(inst, pred, and, zero));
        }
    }
    None
}

/// `X ^ Y --> (X | Y) & !(X & Y)`: where `or` and `and` of the pair fold to
/// the two compares themselves, `lhs & !rhs` (or `!lhs & rhs`) with the
/// predicate of the inverted one flipped in place.
fn fold_by_truth_table(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let env = c.env;
    let simplify = |c: &Combiner, op: BinaryOp| {
        simplify_binop_at(c.context, env.globals, env.layout, Some(&env.analyses), c.function, inst, op, lhs, rhs)
    };
    let or_icmp = simplify(c, BinaryOp::Or)?;
    let and_icmp = simplify(c, BinaryOp::And)?;
    let mut pair = None;
    if or_icmp == lhs && and_icmp == rhs {
        pair = Some(rhs);
    }
    if or_icmp == rhs && and_icmp == lhs {
        pair = Some(lhs);
    }
    let y = pair?;
    let y_inst = c.definition_of(y)?;
    let Opcode::ICmp(pred) = c.function.instruction(y_inst).opcode else { return None };
    let one_use = has_one_use(c, y);
    if !(one_use || c.can_freely_invert_all_users_of(y_inst, Some(inst))) {
        return None;
    }
    // Invert the predicate of 'y', thus inverting its output.
    c.function.set_icmp_predicate(y_inst, pred.inverse());
    c.add_to_worklist(y_inst);
    if !one_use {
        // The other users read what 'y' was.
        let ty = c.type_of(y);
        let ones = c.all_ones(ty);
        let not = c.insert_after(y_inst, Opcode::Binary(BinaryOp::Xor), ty, vec![y, ones]);
        let not_value = c.value_of(not);
        if let Operand::Value(value) = y {
            let uses: Vec<(InstId, u32)> = c.function.users(value).iter().map(|one| (one.user, one.index)).collect();
            for (user, index) in uses {
                if user != not {
                    c.function.set_operand(user, index as usize, not_value);
                    c.add_to_worklist(user);
                }
            }
        }
    }
    Some(c.create_binop(inst, BinaryOp::And, lhs, rhs, Flags::default()))
}

/// `foldXorOfICmps` when both operands of the `xor` are compares.
pub fn fold_xor_of_icmps(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (pred_l, mut lhs0, mut lhs1, pred_r, rhs0, rhs1, width) = {
        let q = query(c, Some(inst));
        let (_, _, pred_l, lhs0, lhs1) = pattern::icmp(&q, op0)?;
        let (_, _, pred_r, rhs0, rhs1) = pattern::icmp(&q, op1)?;
        (pred_l.pred, lhs0, lhs1, pred_r.pred, rhs0, rhs1, q.scalar_bits(lhs0))
    };
    let mut pred_lhs = pred_l;

    if predicates_foldable(pred_lhs, pred_r) {
        if lhs0 == rhs1 && lhs1 == rhs0 {
            pred_lhs = pred_lhs.swapped();
            std::mem::swap(&mut lhs0, &mut lhs1);
        }
        if lhs0 == rhs0 && lhs1 == rhs1 {
            // (icmp1 A, B) ^ (icmp2 A, B) --> (icmp3 A, B)
            let code = icmp_code(pred_lhs) ^ icmp_code(pred_r);
            let sign = is_signed(pred_lhs) || is_signed(pred_r);
            return Some(Visit::Replace(new_icmp_value(c, inst, code, sign, lhs0, lhs1)));
        }
    }

    // The constants are read after the swap, as the C++ reads its swapped
    // `LHS1`.
    let (lc, rc) = {
        let q = query(c, Some(inst));
        (q.int_constant(lhs1).map(|(bits, _)| bits), q.int_constant(rhs1).map(|(bits, _)| bits))
    };
    if let (Some(lc), Some(rc)) = (lc, rc)
        && c.type_of(lhs0) == c.type_of(rhs0)
        && c.context.types.int_bits(c.type_of(lhs0)).is_some()
        && let Some(found) = fold_with_constants(c, inst, (op0, op1), (pred_lhs, lhs0, lc), (pred_r, rhs0, rc), width)
    {
        return Some(Visit::Replace(found));
    }

    fold_by_truth_table(c, inst, op0, op1).map(Visit::Replace)
}

#[cfg(test)]
#[path = "combine_xor_icmps_tests.rs"]
mod tests;
