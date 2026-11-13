//! `foldSelectValueEquivalence`: `x == y ? f(x) : z` where the equality tells
//! one arm what the other computes.
//!
//! Translation of
//! [`InstCombinerImpl::foldSelectValueEquivalence`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L1704)
//! at d1106deb71cc.
//!
//! MIRROR: the three steps in the pin's order: substitute into the true arm,
//! substitute into the false arm without refinement while dropping the
//! poison-generating flags of what it simplified through, and the
//! `trunc nuw` bool form.
//!
//! DEVIATE 1: integers only; `canReplacePointersIfEqual` is not translated
//! (`simplify_select.rs` DEVIATE).
//!
//! DEVIATE 2: `replaceInInstruction` (the direct replacement of a use of the
//! left side by a constant) is not made: see `combine_visit_select` DEVIATE 3.
//!
//! # Example
//! ```text
//! %c = icmp eq i32 %x, 2147483647
//! %a = add nsw i32 %x, 1                     %r = add i32 %x, 1
//! %r = select i1 %c, i32 -2147483648, i32 %a   ==>
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::simplify::{simplify_with_op_replaced_at, simplify_with_op_replaced_dropping_flags_at};
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef;
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{drop_poison_generating_annotations, query, replace_operand};
use crate::combine_visit_select::Sel;

/// `foldSelectValueEquivalence(SI, *Cmp)` for an `icmp eq` or `icmp ne`.
pub(crate) fn select_value_equivalence(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let (pred, lhs, rhs) = {
        let q = query(c, None);
        let (_, _, pred, lhs, rhs) = pattern::icmp(&q, sel.cond)?;
        (pred.pred, lhs, rhs)
    };
    c.context.types.int_bits(c.type_of(lhs))?;
    // Canonicalize the pattern to an equivalence on the predicate by swapping
    // the select operands.
    let (true_val, false_val, swapped) = match pred {
        IntPredicate::Eq => (sel.if_true, sel.if_false, false),
        IntPredicate::Ne => (sel.if_false, sel.if_true, true),
        _ => return None,
    };
    for (old, new) in [(lhs, rhs), (rhs, lhs)] {
        if let Some(found) = replace_old_op_with_new_op(c, inst, true_val, swapped, old, new) {
            return Some(found);
        }
    }
    c.definition_of(false_val)?;
    // InstSimplify already performed this fold if it was possible subject to
    // current poison-generating flags. Check whether dropping poison-generating
    // flags enables the transform.
    for (old, new) in [(lhs, rhs), (rhs, lhs)] {
        let mut drop_flags = Vec::new();
        let simplified = {
            let env = c.env;
            simplify_with_op_replaced_dropping_flags_at(
                c.context,
                env.globals,
                env.layout,
                Some(&env.analyses),
                c.function,
                inst,
                false_val,
                old,
                new,
                false,
                &mut drop_flags,
            )
        };
        if simplified.is_some_and(|value| c.same_constant(value, true_val)) {
            for dropped in drop_flags {
                drop_poison_generating_annotations(c, dropped);
                c.worklist.add(dropped);
            }
            return Some(Visit::Replace(false_val));
        }
    }
    // select (x == C), (C != 0), (trunc nuw x to i1) is x != 0.
    let q = query(c, None);
    let (_, _, x) = pattern::cast(&q, false_val, CastOp::Trunc)?;
    let flags = c.function.instruction(c.definition_of(false_val)?).flags;
    let cmp_c = if x == lhs { rhs } else { return None };
    if c.context.types.int_bits(sel.ty) != Some(1) || !flags.contains(Flags::NUW) || !pattern::imm_constant(&q, cmp_c) {
        return None;
    }
    let (value, _) = q.int_constant(cmp_c)?;
    let ne_zero = c.int(sel.ty, u128::from(value != 0));
    if !c.same_constant(ne_zero, true_val) {
        return None;
    }
    let ty = c.type_of(lhs);
    let zero = c.int(ty, 0);
    let i1 = c.context.types.int(1);
    Some(c.replace_with_new(inst, Opcode::ICmp(IntPredicate::Ne), i1, vec![lhs, zero], Flags::default()))
}

/// `ReplaceOldOpWithNewOp`: in `X == Y ? f(X) : Z`, try to evaluate `f(Y)`
/// and replace the arm.
fn replace_old_op_with_new_op(
    c: &mut Combiner,
    inst: InstId,
    true_val: Operand,
    swapped: bool,
    old: Operand,
    new: Operand,
) -> Option<Visit> {
    let (old_is_constant, new_is_constant) = {
        let q = query(c, None);
        (pattern::imm_constant(&q, old), pattern::imm_constant(&q, new))
    };
    // Take care to avoid replacing X == Y ? X : Z with X == Y ? Y : Z, as that
    // would lead to an infinite replacement cycle.
    if true_val == old && (old_is_constant || !new_is_constant) {
        return None;
    }
    let simplified = {
        let env = c.env;
        simplify_with_op_replaced_at(
            c.context,
            env.globals,
            env.layout,
            Some(&env.analyses),
            c.function,
            inst,
            true_val,
            old,
            new,
        )
    }?;
    let slot = if swapped { 2 } else { 1 };
    let not_undef = |c: &Combiner, value: Operand| is_guaranteed_not_to_be_undef(&query(c, Some(inst)), value, 0);
    // If we simplify to a constant, replace if we aren't creating new undef.
    if pattern::imm_constant(&query(c, None), simplified) && not_undef(c, simplified) {
        replace_operand(c, inst, slot, simplified);
        return Some(Visit::Changed);
    }
    // If NewOp is a constant and OldOp is not replace iff NewOp doesn't contain
    // undef elements. Make sure that V is always simpler than TrueVal,
    // otherwise we might end up in an infinite loop.
    let operand_of_true =
        c.definition_of(true_val).is_some_and(|def| c.function.instruction(def).operands.contains(&simplified));
    if new_is_constant || operand_of_true {
        if not_undef(c, new) {
            replace_operand(c, inst, slot, simplified);
            return Some(Visit::Changed);
        }
        return None;
    }
    None
}

#[cfg(test)]
#[path = "combine_select_equivalence_tests.rs"]
mod tests;
