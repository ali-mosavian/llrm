//! The select folds of `visitSelectInst` that rewrite a select whose arms or
//! condition are themselves selects:
//! [`foldSelectOfSymmetricSelect`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L3611)
//! and
//! [`foldNestedSelects`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L3639).
//!
//! # How it works
//! `select c2, (select c1, a, b), (select c1, b, a)` is
//! `select (xor c1, c2), b, a`. A select whose condition is a logical and/or
//! of its inner select's condition and another `alt` is two selects with the
//! inner condition outermost: the inner select moves into the arm that `alt`
//! decides.
//!
//! # Example
//! ```text
//! %c = select i1 %a, i1 %b, i1 false        %s = select i1 %b, i8 %x, i8 %y
//! %i = select i1 %a, i8 %y, i8 %z    ==>    %r = select i1 %a, i8 %s, i8 %z
//! %r = select i1 %c, i8 %x, i8 %i
//! ```
//!
//! MIRROR: the pattern, the one-use guards, the inversion swaps and the order
//! of the two folds.
//!
//! DEVIATE 1: the new instruction is not named after the old one
//! (`takeName`): MIR names are not read by any pass.
//!
//! # Used by
//! `combine_visit_select` (`FOLDS`).

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;
use crate::combine_visit_select::Sel;

/// `select c2, (select c1, a, b), (select c1, b, a)` is
/// `select (xor c1, c2), b, a`.
pub(crate) fn select_of_symmetric_select(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let (_, [inner_cond, inner_true, inner_false]) = c.select_of(sel.if_true)?;
    let (_, [other_cond, other_true, other_false]) = c.select_of(sel.if_false)?;
    if !c.has_one_use(sel.if_true)
        || !c.has_one_use(sel.if_false)
        || other_cond != inner_cond
        || other_true != inner_false
        || other_false != inner_true
        || c.type_of(sel.cond) != c.type_of(inner_cond)
    {
        return None;
    }
    let xor = c.create_binop(inst, BinaryOp::Xor, inner_cond, sel.cond, Flags::default());
    let ty = c.type_of(inner_false);
    Some(c.replace_with_new(inst, Opcode::Select, ty, vec![xor, inner_false, inner_true], Flags::default()))
}

/// `m_c_LogicalAnd`/`m_c_LogicalOr` with one side `wanted`: the other side.
fn commutative_logical(
    c: &Combiner,
    at: InstId,
    v: Operand,
    and: bool,
    wanted: impl Fn(&Query, Operand) -> bool,
) -> Option<Operand> {
    let q = query(c, Some(at));
    let (a, b) = if and { pattern::logical_and(&q, v)? } else { pattern::logical_or(&q, v)? };
    if wanted(&q, a) {
        Some(b)
    } else if wanted(&q, b) {
        Some(a)
    } else {
        None
    }
}

/// `foldNestedSelects`: `select (and/or c1, alt), ..., (select c1, ..)` is the
/// inner condition outermost.
pub(crate) fn nested_selects(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let (mut cond, mut outer_true, mut outer_false) = (sel.cond, sel.if_true, sel.if_false);
    // Canonicalize inversion of the outermost select's condition.
    let not = {
        let q = query(c, Some(inst));
        pattern::not(&q, cond)
    };
    if let Some(inner) = not {
        cond = inner;
        std::mem::swap(&mut outer_true, &mut outer_false);
    }
    // The condition of the outermost select must be an and/or.
    let (is_and, is_logical) = {
        let q = query(c, Some(inst));
        let and = pattern::logical_and(&q, cond).is_some();
        (and, and || pattern::logical_or(&q, cond).is_some())
    };
    if !is_logical {
        return None;
    }
    // Depending on the logical op, the inner select is in a different hand.
    let inner_value = if is_and { outer_false } else { outer_true };
    // Profitability: do not increase the instruction count.
    if !c.has_one_use(sel.cond) && !c.has_one_use(inner_value) {
        return None;
    }
    let (_, [mut inner_cond, mut inner_true, mut inner_false]) = c.select_of(inner_value)?;
    let not = {
        let q = query(c, Some(inst));
        pattern::not(&q, inner_cond)
    };
    if let Some(inner) = not {
        inner_cond = inner;
        std::mem::swap(&mut inner_true, &mut inner_false);
    }
    let alt = if let Some(alt) = commutative_logical(c, inst, cond, is_and, |_, o| o == inner_cond) {
        alt
    } else if let Some(alt) = commutative_logical(c, inst, cond, is_and, |q, o| pattern::not(q, o) == Some(inner_cond))
    {
        // `cond` holds the negated inner condition: its own value stands for
        // it.
        let negated = {
            let q = query(c, Some(inst));
            let (a, b) = if is_and { pattern::logical_and(&q, cond)? } else { pattern::logical_or(&q, cond)? };
            if pattern::not(&q, a) == Some(inner_cond) { a } else { b }
        };
        std::mem::swap(&mut inner_true, &mut inner_false);
        inner_cond = negated;
        alt
    } else {
        return None;
    };
    let (alt_true, alt_false) = if is_and { (outer_true, inner_true) } else { (inner_false, outer_false) };
    let inner = c.create_select(inst, alt, alt_true, alt_false);
    let (arm_true, arm_false) = if is_and { (inner, inner_false) } else { (inner_true, inner) };
    let ty = c.type_of(arm_true);
    Some(c.replace_with_new(inst, Opcode::Select, ty, vec![inner_cond, arm_true, arm_false], Flags::default()))
}

#[cfg(test)]
#[path = "combine_select_tail_tests.rs"]
mod tests;
