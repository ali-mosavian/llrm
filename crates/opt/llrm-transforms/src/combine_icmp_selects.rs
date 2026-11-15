//! `visitICmpInst`'s compare of two selects on one condition: when the compare
//! of one pair of arms simplifies, only the other pair is compared and the
//! condition picks the result.
//!
//! Translation of
//! [`InstCombinerImpl::visitICmpInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L8128)
//! at d1106deb71cc.
//!
//! MIRROR: the two selects share the condition and one has no other user.
//!
//! # Example
//! ```text
//! %x = select i1 %c, i8 %q, i8 2          %i = icmp ugt i8 %p, %q
//! %y = select i1 %c, i8 %p, i8 0          %r = select i1 %c, i1 %i, i1 false
//! %r = icmp ult i8 %x, %y          ==>
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{Flags, IntPredicate};
use llrm_mir::simplify::simplify_icmp_operands;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::has_one_use;

pub(crate) fn fold_icmp_of_selects(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (first, [cond, a, b]) = c.select_of(op0)?;
    let (_, [other_cond, cc, d]) = c.select_of(op1)?;
    if cond != other_cond || !(has_one_use(c, op0) || has_one_use(c, op1)) {
        return None;
    }
    let simplified =
        |c: &Combiner, x: Operand, y: Operand| simplify_icmp_operands(c.context, c.env.layout, c.function, pred, x, y);
    // Whether the compare of the true arms simplifies.
    if let Some(res) = simplified(c, a, cc) {
        let rest = c.create_icmp(inst, pred, b, d);
        return Some(Visit::Replace(c.select_like(inst, first, [cond, res, rest], Flags::default())));
    }
    // Or of the false arms.
    if let Some(res) = simplified(c, b, d) {
        let rest = c.create_icmp(inst, pred, a, cc);
        return Some(Visit::Replace(c.select_like(inst, first, [cond, rest, res], Flags::default())));
    }
    None
}

#[cfg(test)]
#[path = "combine_icmp_selects_tests.rs"]
mod tests;
