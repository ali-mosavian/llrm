//! `visitSelectInst`: a select of the two values an unordered `fcmp` compares
//! takes the ordered compare and swapped arms.
//!
//! Translation of the `FCmpInst` block of
//! [`visitSelectInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L4693)
//! at d1106deb71cc.
//!
//! MIRROR: the new compare is the inverse predicate with the old compare's
//! fast-math flags (named `.inv` in the pin); `nnan` and `ninf` of the compare
//! reach the select; the old compare must have one use.
//!
//! DEVIATE 1: only this canonicalisation of the block is translated (the
//! scale-if-equals-zero, `minnum`/`maxnum` and `canonicalize` folds after it
//! are not).
//!
//! # Used by
//! `visitSelectInst`.
//!
//! # Example
//! ```text
//! %c = fcmp ult float %p, %q                      %c.inv = fcmp oge float %p, %q
//! %r = select i1 %c, float %p, float %q    ==>    %r = select i1 %c.inv, float %q, float %p
//! ```

use llrm_mir::module::InstId;
use llrm_mir::opcode::{Flags, Opcode};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};
use crate::combine_visit_select::Sel;

/// (X ugt Y) ? X : Y -> (X ole Y) ? Y : X.
pub(crate) fn select_fcmp_unordered_swap(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let cond = query(c, None).instruction_of(sel.cond).map(|(_, one)| one.clone())?;
    let Opcode::FCmp(pred) = cond.opcode else { return None };
    let [lhs, rhs] = cond.operands[..] else { return None };
    let same_pair = (lhs == sel.if_true && rhs == sel.if_false) || (lhs == sel.if_false && rhs == sel.if_true);
    if !same_pair || !has_one_use(c, sel.cond) || !pred.is_unordered() {
        return None;
    }
    let fmf = cond.flags.intersect(Flags::FAST);
    let ty = c.context.types.int(1);
    let new = c.insert_before(inst, Opcode::FCmp(pred.inverse()), ty, vec![lhs, rhs], fmf);
    let new_cond = c.value_of(new);
    // Propagate ninf/nnan from fcmp to select.
    let flags = c
        .function
        .instruction(inst)
        .flags
        .intersect(Flags::FAST)
        .union(cond.flags.intersect(Flags::NNAN))
        .union(cond.flags.intersect(Flags::NINF));
    let select = c.select_like(inst, inst, [new_cond, sel.if_false, sel.if_true], flags);
    Some(Visit::Replace(select))
}

#[cfg(test)]
#[path = "combine_select_fcmp_swap_tests.rs"]
mod tests;
