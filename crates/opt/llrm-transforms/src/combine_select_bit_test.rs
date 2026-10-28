//! `foldSelectBitTest`: `select (icmp eq (and x, 2^k), 0), TC, FC` as the
//! `and` shifted and combined with a constant.
//!
//! Translation of
//! [`foldSelectBitTest`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L4422)
//! and
//! [`foldSelectICmpAnd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L153)
//! at d1106deb71cc.
//!
//! DEVIATE 1: no vectors.
//!
//! DEVIATE 2 (temporary): `foldSelectICmpAndBinOp` is not made.
//!
//! # Example
//! ```text
//! %a = and i32 %v, 2                          %a = and i32 %v, 2
//! %c = icmp eq i32 %a, 0               ==>    %r = xor i32 %a, 6
//! %r = select i1 %c, i32 6, i32 4
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate};
use llrm_mir::valuetracking::bittest::{BitTestOptions, decompose_bit_test};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};
use crate::combine_visit_select::Sel;

fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// The bit test a condition is: `v & and_mask` against zero, and whether the
/// `and` is not in the program yet.
struct BitTest {
    v: Operand,
    and_mask: u128,
    create_and: bool,
    pred: IntPredicate,
}

pub(crate) fn select_bit_test(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let q = query(c, Some(inst));
    let test = if let Some((_, _, pred, lhs, rhs)) = pattern::icmp(&q, sel.cond) {
        if matches!(pred.pred, IntPredicate::Eq | IntPredicate::Ne) {
            if !pattern::zero(&q, rhs) {
                return None;
            }
            let (_, _, a, b) = pattern::binary(&q, lhs, BinaryOp::And)?;
            let and_mask = pattern::power2_constant(&q, b).or_else(|| pattern::power2_constant(&q, a))?;
            BitTest { v: lhs, and_mask, create_and: false, pred: pred.pred }
        } else {
            let found = decompose_bit_test(&q, sel.cond, BitTestOptions::DEFAULT)?;
            let known = llrm_mir::valuetracking::compute_known_bits(c.context, c.function, found.x, 0, Some(inst));
            let and_mask = found.mask & known.max_value();
            if !and_mask.is_power_of_two() {
                return None;
            }
            BitTest { v: found.x, and_mask, create_and: true, pred: found.pred }
        }
    } else {
        let (_, trunc, x) = pattern::cast(&q, sel.cond, CastOp::Trunc)?;
        BitTest { v: x, and_mask: 1, create_and: !trunc.flags.contains(Flags::NUW), pred: IntPredicate::Ne }
    };
    let (if_true, if_false) =
        if test.pred == IntPredicate::Ne { (sel.if_false, sel.if_true) } else { (sel.if_true, sel.if_false) };
    let (tc, sel_bits) = q.int_constant(if_true)?;
    let (fc, _) = q.int_constant(if_false)?;
    let value = fold_select_icmp_and(c, inst, sel, &test, tc, fc, sel_bits)?;
    Some(Visit::Replace(value))
}

/// `foldSelectICmpAnd`: `(v & m) == 0 ? tc : fc`, `m` a power of two and
/// `tc - fc` too (or one arm zero).
fn fold_select_icmp_and(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
    test: &BitTest,
    tc: u128,
    fc: u128,
    sel_bits: u32,
) -> Option<Operand> {
    let v_bits = c.context.types.int_bits(c.type_of(test.v))?;
    let cond_one_use = has_one_use(c, sel.cond);
    if tc != 0 && fc != 0 {
        if sel_bits != v_bits {
            return None;
        }
        // If we have to create an 'and', then we must kill the cmp to not
        // increase the instruction count.
        if test.create_and && !cond_one_use {
            return None;
        }
        let m = mask(sel_bits);
        for op in [BinaryOp::Or, BinaryOp::Xor, BinaryOp::Add, BinaryOp::Sub] {
            let folded = match op {
                BinaryOp::Or => tc | test.and_mask,
                BinaryOp::Xor => tc ^ test.and_mask,
                BinaryOp::Add => tc.wrapping_add(test.and_mask) & m,
                _ => tc.wrapping_sub(test.and_mask) & m,
            };
            if folded == fc {
                let mut v = test.v;
                if test.create_and {
                    let mask_c = c.int(sel.ty, test.and_mask);
                    v = c.create_binop(inst, BinaryOp::And, v, mask_c, Flags::default());
                }
                let tcc = c.int(sel.ty, tc);
                return Some(c.create_binop(inst, op, tcc, v, Flags::default()));
            }
        }
        return None;
    }

    // Make sure one of the select arms is a power-of-2.
    if !tc.is_power_of_two() && !fc.is_power_of_two() {
        return None;
    }
    let val = if tc != 0 { tc } else { fc };
    let val_zeros = val.trailing_zeros();
    let and_zeros = test.and_mask.trailing_zeros();
    let should_not_val = tc != 0;
    let need_shift = val_zeros != and_zeros;
    let need_ext_trunc = sel_bits != v_bits;
    // Creating an 'and', 'shift', 'xor' and cast to replace a 'select' and
    // 'icmp' would not decrease the instruction count.
    let created =
        u32::from(test.create_and) + u32::from(should_not_val) + u32::from(need_shift) + u32::from(need_ext_trunc);
    if created > 1 + u32::from(cond_one_use) {
        return None;
    }
    let mut v = test.v;
    if test.create_and {
        let mask_c = c.int(c.type_of(v), test.and_mask);
        v = c.create_binop(inst, BinaryOp::And, v, mask_c, Flags::default());
    }
    if val_zeros > and_zeros {
        v = zext_or_trunc(c, inst, v, sel, v_bits);
        let by = c.int(sel.ty, u128::from(val_zeros - and_zeros));
        v = c.create_binop(inst, BinaryOp::Shl, v, by, Flags::default());
    } else if val_zeros < and_zeros {
        let by = c.int(c.type_of(v), u128::from(and_zeros - val_zeros));
        v = c.create_binop(inst, BinaryOp::LShr, v, by, Flags::default());
        v = zext_or_trunc(c, inst, v, sel, v_bits);
    } else {
        v = zext_or_trunc(c, inst, v, sel, v_bits);
    }
    // Whether the compare was an icmp_ne or icmp_eq and which arm is zero.
    if should_not_val {
        let val_c = c.int(sel.ty, val);
        v = c.create_binop(inst, BinaryOp::Xor, v, val_c, Flags::default());
    }
    Some(v)
}

/// `Builder.CreateZExtOrTrunc(v, select type)`.
fn zext_or_trunc(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
    sel: &Sel,
    from_bits: u32,
) -> Operand {
    let to_bits = c.context.types.int_bits(sel.ty).unwrap_or(from_bits);
    match to_bits.cmp(&from_bits) {
        std::cmp::Ordering::Equal => v,
        std::cmp::Ordering::Greater => c.create_cast(at, CastOp::ZExt, v, sel.ty, Flags::default()),
        std::cmp::Ordering::Less => c.create_cast(at, CastOp::Trunc, v, sel.ty, Flags::default()),
    }
}

#[cfg(test)]
#[path = "combine_select_bit_test_tests.rs"]
mod tests;
