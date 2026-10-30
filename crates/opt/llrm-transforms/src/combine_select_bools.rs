//! `foldSelectOfBools`: a `select` of `i1` as a logical operation.
//!
//! Translation of
//! [`InstCombinerImpl::foldSelectOfBools`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L3754)
//! at d1106deb71cc, in its order, each fold a function in [`FOLDS`].
//!
//! MIRROR: `select c, true, x` is `or c, x` where `x` is poison only if `c`
//! is; `select c, x, false` is `and`; the nested logical and/or reassociate
//! (`(A || B) || C` as `A || (B | C)`); a constant arm moves the condition's
//! negation; De Morgan in select form; a select of a select that decides
//! itself.
//!
//! DEVIATE 1: `impliesPoisonOrCond` is `impliesPoison` alone: the `samesign`
//! compare and `trunc nuw` cases are not found (it finds less).
//!
//! DEVIATE 2 (temporary): the folds that need another family's helper are left
//! out, named where they stood: the factorization of `(A && B) || (C && B)`,
//! `~(A & B) & (A | B)`, the `select (~a | c)`, `(c & b)`, `(a | c)` and
//! `(c & ~b)` forms (`getFreelyInverted`), `isCheckForZeroAndMulWithOverflow`,
//! `foldBooleanAndOr` (and/or/xor family), the implied-condition
//! simplifications and `FoldOrOfLogicalAnds`.
//!
//! DEVIATE 3: `prof` branch weights are dropped where the arms swap.
//!
//! # Example
//! ```text
//! %r = select i1 %a, i1 true, i1 %b        %r = or i1 %a, %b       ; %b is poison only if %a is
//! ```

use llrm_mir::module::InstId;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{BinaryOp, Flags};
use llrm_mir::valuetracking::guarantee::implies_poison;
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query, replace_operand};
use crate::combine_visit_select::{Fold, Sel};

/// The rewrites in the pin's order.
const FOLDS: &[Fold] = &[
    or_of_condition,
    logical_or_reassociated,
    and_of_condition,
    logical_and_reassociated,
    false_true_arm_negated,
    true_false_arm_negated,
    de_morgan_and,
    de_morgan_or,
    select_deciding_itself_true,
    select_deciding_itself_false,
];

/// `foldSelectOfBools`: the first of `FOLDS` that applies to an `i1` select
/// on a non-constant condition.
pub(crate) fn select_of_bools(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    // Avoid infinite loops: a non-constant condition of the select's type.
    if c.context.types.int_bits(sel.ty) != Some(1)
        || matches!(sel.cond, Operand::Constant(_))
        || c.type_of(sel.if_true) != c.type_of(sel.cond)
    {
        return None;
    }
    FOLDS.iter().find_map(|fold| fold(c, inst, sel))
}

impl Combiner<'_> {
    fn is_true(
        &self,
        v: Operand,
    ) -> bool {
        let one = self.int(self.type_of(v), 1);
        self.same_constant(v, one)
    }

    fn is_false(
        &self,
        v: Operand,
    ) -> bool {
        let zero = self.int(self.type_of(v), 0);
        self.same_constant(v, zero)
    }

    /// `impliesPoisonOrCond(assumed, v, ..)` (DEVIATE 1).
    fn implies_poison_or_cond(
        &self,
        inst: InstId,
        assumed_poison: Operand,
        v: Operand,
    ) -> bool {
        implies_poison(&query(self, Some(inst)), assumed_poison, v)
    }
}

/// `select c, true, x` is `or c, x`.
fn or_of_condition(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if !c.is_true(sel.if_true) || !c.implies_poison_or_cond(inst, sel.if_false, sel.cond) {
        return None;
    }
    Some(c.replace_with_binop(inst, BinaryOp::Or, sel.cond, sel.if_false, Flags::default()))
}

/// `(A || B) || C` is `A || (B | C)`.
fn logical_or_reassociated(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if !c.is_true(sel.if_true) || !has_one_use(c, sel.cond) {
        return None;
    }
    let (_, [a, inner_true, b]) = c.select_of(sel.cond)?;
    if !c.is_true(inner_true) || !c.implies_poison_or_cond(inst, sel.if_false, b) {
        return None;
    }
    let either = c.create_binop(inst, BinaryOp::Or, b, sel.if_false, Flags::default());
    Some(Visit::Replace(c.create_logical_or(inst, inst, a, either)))
}

/// `select c, x, false` is `and c, x`.
fn and_of_condition(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if !c.is_false(sel.if_false) || !c.implies_poison_or_cond(inst, sel.if_true, sel.cond) {
        return None;
    }
    Some(c.replace_with_binop(inst, BinaryOp::And, sel.cond, sel.if_true, Flags::default()))
}

/// `(A && B) && C` is `A && (B & C)`.
fn logical_and_reassociated(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if !c.is_false(sel.if_false) || !has_one_use(c, sel.cond) {
        return None;
    }
    let (_, [a, b, inner_false]) = c.select_of(sel.cond)?;
    if !c.is_false(inner_false) || !c.implies_poison_or_cond(inst, sel.if_true, b) {
        return None;
    }
    let both = c.create_binop(inst, BinaryOp::And, b, sel.if_true, Flags::default());
    Some(Visit::Replace(c.create_logical_and(inst, inst, a, both)))
}

/// `select a, false, b` is `select !a, b, false`.
fn false_true_arm_negated(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if !c.is_false(sel.if_true) {
        return None;
    }
    let not_cond = c.create_not(inst, sel.cond);
    let zero = sel.if_true;
    let new = c.select_like(inst, inst, [not_cond, sel.if_false, zero], Flags::default());
    c.function.set_metadata(c.definition_of(new)?, "prof", None);
    Some(Visit::Replace(new))
}

/// `select a, b, true` is `select !a, true, b`.
fn true_false_arm_negated(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if !c.is_true(sel.if_false) {
        return None;
    }
    let not_cond = c.create_not(inst, sel.cond);
    let one = sel.if_false;
    let new = c.select_like(inst, inst, [not_cond, one, sel.if_true], Flags::default());
    c.function.set_metadata(c.definition_of(new)?, "prof", None);
    Some(Visit::Replace(new))
}

/// `select !a, !b, false` is `not (select a, true, b)`.
fn de_morgan_and(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if !c.is_false(sel.if_false) || !(has_one_use(c, sel.cond) || has_one_use(c, sel.if_true)) {
        return None;
    }
    let (a, b) = {
        let q = query(c, Some(inst));
        (pattern::not(&q, sel.cond)?, pattern::not(&q, sel.if_true)?)
    };
    if matches!(a, Operand::Constant(_)) || matches!(b, Operand::Constant(_)) {
        return None;
    }
    let one = c.int(c.type_of(a), 1);
    let either = c.select_like(inst, inst, [a, one, b], Flags::default());
    c.function.set_metadata(c.definition_of(either)?, "prof", None);
    let ones = c.all_ones(c.type_of(either));
    Some(c.replace_with_binop(inst, BinaryOp::Xor, either, ones, Flags::default()))
}

/// `select !a, true, !b` is `not (select a, b, false)`.
fn de_morgan_or(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if !c.is_true(sel.if_true) || !(has_one_use(c, sel.cond) || has_one_use(c, sel.if_false)) {
        return None;
    }
    let (a, b) = {
        let q = query(c, Some(inst));
        (pattern::not(&q, sel.cond)?, pattern::not(&q, sel.if_false)?)
    };
    if matches!(a, Operand::Constant(_)) || matches!(b, Operand::Constant(_)) {
        return None;
    }
    let zero = c.int(c.type_of(a), 0);
    let both = c.select_like(inst, inst, [a, b, zero], Flags::default());
    c.function.set_metadata(c.definition_of(both)?, "prof", None);
    let ones = c.all_ones(c.type_of(both));
    Some(c.replace_with_binop(inst, BinaryOp::Xor, both, ones, Flags::default()))
}

/// `select (select a, true, b), true, b` is `select a, true, b`.
fn select_deciding_itself_true(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let (_, [a, inner_true, b]) = c.select_of(sel.cond)?;
    if !(c.is_true(inner_true) && c.is_true(sel.if_true) && sel.if_false == b) {
        return None;
    }
    replace_operand(c, inst, 0, a);
    Some(Visit::Changed)
}

/// `select (select a, b, false), b, false` is `select a, b, false`.
fn select_deciding_itself_false(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let (_, [a, b, inner_false]) = c.select_of(sel.cond)?;
    if !(c.is_false(inner_false) && sel.if_true == b && c.is_false(sel.if_false)) {
        return None;
    }
    replace_operand(c, inst, 0, a);
    Some(Visit::Changed)
}

#[cfg(test)]
#[path = "combine_select_bools_tests.rs"]
mod tests;
