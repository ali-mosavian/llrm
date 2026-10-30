//! `foldSelectInstWithICmp`'s sign-bit step: a select on `x > -1` tests
//! `x < 0` with its arms swapped.
//!
//! Translation of
//! [the step at
//! :2449](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2449)
//! of
//! [`foldSelectInstWithICmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2419)
//! at d1106deb71cc.
//!
//! MIRROR: no constant arm, and a compare with one use (the pin's guards
//! against looping with other canonicalizations).
//!
//! DEVIATE 1: the steps of `foldSelectInstWithICmp` before this one
//! (`foldSelectInstWithICmpConst`, `canonicalizeClampLike`,
//! `tryToReuseConstantFromSelectInComparison`,
//! `foldICmpUSubSatWithAndForMostSignificantBitCmp`, `foldSelectICmpEq`) and
//! after it (`foldSelectICmpMinMax` ... `foldSelectWithConstOpToBinOp`) are
//! not translated.
//!
//! DEVIATE 2: the select's `prof` branch weights are dropped, not swapped.
//!
//! # Used by
//! `visitSelectInst`, right after `canonicalizeSPF`.
//!
//! # Example
//! ```text
//! %c = icmp sgt i16 %x, -1                  %c = icmp slt i16 %x, 0
//! %r = select i1 %c, i16 %a, i16 %b   ==>   %r = select i1 %c, i16 %b, i16 %a
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::IntPredicate;
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query, replace_operand};
use crate::combine_visit_select::Sel;

/// `(CmpLHS > -1) ? TV : FV --> (CmpLHS < 0) ? FV : TV`.
pub(crate) fn select_sign_bit_condition(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let lhs = {
        let q = query(c, Some(inst));
        let (_, _, found, lhs, rhs) = pattern::icmp(&q, sel.cond)?;
        if found.pred != IntPredicate::Sgt || !pattern::all_ones(&q, rhs) {
            return None;
        }
        lhs
    };
    // To avoid conflicts (infinite loops) with other canonicalizations, this
    // is not applied with any constant select arm.
    if matches!(sel.if_true, Operand::Constant(_)) || matches!(sel.if_false, Operand::Constant(_)) {
        return None;
    }
    if !has_one_use(c, sel.cond) {
        return None;
    }
    let zero = c.int(c.type_of(lhs), 0);
    let is_neg = c.create_icmp(inst, IntPredicate::Slt, lhs, zero);
    replace_operand(c, inst, 0, is_neg);
    c.function.set_operand(inst, 1, sel.if_false);
    c.function.set_operand(inst, 2, sel.if_true);
    c.function.set_metadata(inst, "prof", None);
    Some(Visit::Changed)
}

#[cfg(test)]
#[path = "combine_select_sign_bit_tests.rs"]
mod tests;
