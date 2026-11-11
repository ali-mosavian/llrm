//! `FoldSelectWithAndOrCond`: a select on an `and` or `or` of two conditions,
//! when one of them settles the arms.
//!
//! Translation of the lambda in
//! [`visitSelectInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L5250)
//! and its dispatch on the condition
//! ([:5296](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L5296))
//! at d1106deb71cc.
//!
//! # How it works
//! - `select (A && B), T, F` is `select A, (select B, T, F), F` and `select (A
//!   || B), T, F` is `select A, T, (select B, T, F)`, when the inner select
//!   simplifies. A plain `and`/`or` tries both operand orders; a logical
//!   (select-form) one only the written order.
//!
//! # Example
//! ```text
//! %c = or i1 %a, %n ; %t = or i1 %b, %n
//! %r = select i1 %c, i1 %t, i1 false        %r = select i1 %a, i1 %t, i1 %n
//! ```
//!
//! MIRROR: the dispatch order, `simplifySelectInst(B, T, F)` asked at the
//! select, the branch metadata kept only when both arms are unchanged.
//!
//! DEVIATE 1: the `canonicalizeSPF` step on a compare `B` is not here (it is
//! `combine_select_spf`'s next strip).
//!
//! # Used by
//! `visitSelectInst`.

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::simplify::simplify_select_operands_at;
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;
use crate::combine_visit_select::Sel;

/// `FoldSelectWithAndOrCond(IsAnd, A, B)`.
fn fold_with(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
    is_and: bool,
    a: Operand,
    b: Operand,
) -> Option<Visit> {
    let env = c.env;
    let simpler = simplify_select_operands_at(
        c.context,
        env.globals,
        env.layout,
        Some(&env.analyses),
        c.function,
        inst,
        b,
        sel.if_true,
        sel.if_false,
    )?;
    let (new_true, new_false) = if is_and { (simpler, sel.if_false) } else { (sel.if_true, simpler) };
    let operands = vec![a, new_true, new_false];
    if (new_true, new_false) == (sel.if_true, sel.if_false) {
        let new = c.select_like(inst, inst, [a, new_true, new_false], Flags::default());
        return Some(Visit::Replace(new));
    }
    Some(c.replace_with_new(inst, Opcode::Select, sel.ty, operands, Flags::default()))
}

/// The fold on `visitSelectInst`'s condition.
pub(crate) fn select_with_and_or_cond(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let (plain_and, plain_or, logical_and, logical_or) = {
        let q = query(c, Some(inst));
        (
            pattern::binary(&q, sel.cond, BinaryOp::And).map(|(_, _, l, r)| (l, r)),
            pattern::binary(&q, sel.cond, BinaryOp::Or).map(|(_, _, l, r)| (l, r)),
            pattern::logical_and(&q, sel.cond),
            pattern::logical_or(&q, sel.cond),
        )
    };
    if let Some((lhs, rhs)) = plain_and {
        return fold_with(c, inst, sel, true, lhs, rhs).or_else(|| fold_with(c, inst, sel, true, rhs, lhs));
    }
    if let Some((lhs, rhs)) = plain_or {
        return fold_with(c, inst, sel, false, lhs, rhs).or_else(|| fold_with(c, inst, sel, false, rhs, lhs));
    }
    // A logical operation's operands cannot swap.
    if let Some((lhs, rhs)) = logical_and {
        return fold_with(c, inst, sel, true, lhs, rhs);
    }
    if let Some((lhs, rhs)) = logical_or {
        return fold_with(c, inst, sel, false, lhs, rhs);
    }
    None
}

#[cfg(test)]
#[path = "combine_select_andor_tests.rs"]
mod tests;
