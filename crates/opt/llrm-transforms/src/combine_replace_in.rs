//! `replaceInInstruction`: the uses of a value inside the one-use chain that
//! feeds a `select` arm read a constant instead.
//!
//! Translation of
//! [`InstCombinerImpl::replaceInInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L1658)
//! at d1106deb71cc, called from `visitSelectInst` (:4644) with the condition
//! and `true` in the true arm, `false` in the false arm.
//!
//! MIRROR: two instructions up, each with one use and safe to execute with
//! its operands replaced.
//!
//! DEVIATE 1: no lane-crossing check (MIR has no vectors).
//!
//! # Runs
//! In InstCombine's select visit (`combine_visit_select`) and in
//! `combine_select_equivalence`.
//!
//! # Example
//! ```text
//! %b = select i1 %c, i8 %q, i8 2          %b = select i1 true, i8 %q, i8 2
//! %i = icmp ugt i8 %p, %b          ==>    %i = icmp ugt i8 %p, %b
//! %r = select i1 %c, i1 %i, i1 false      %r = select i1 %c, i1 %i, i1 false
//! ```

use llrm_mir::lazyvalueinfo::is_safe_to_speculatively_execute_with_variable_replaced;
use llrm_mir::module::Operand;

use crate::combine::Combiner;
use crate::combine_shared::{query, replace_operand};

/// Conservatively limit replacement to two instructions upwards.
const MAX_DEPTH: u32 = 2;

impl Combiner<'_> {
    /// `replaceInInstruction(V, Old, New, Depth)`: whether a use was replaced.
    pub(crate) fn replace_in_instruction(
        &mut self,
        v: Operand,
        old: Operand,
        new: Operand,
        depth: u32,
    ) -> bool {
        if depth == MAX_DEPTH {
            return false;
        }
        let Some(inst) = self.definition(v) else { return false };
        if !self.has_one_use(v) {
            return false;
        }
        let safe = {
            let q = query(self, Some(inst));
            is_safe_to_speculatively_execute_with_variable_replaced(&q, self.function.instruction(inst), true)
        };
        if !safe {
            return false;
        }
        let mut changed = false;
        for slot in 0..self.function.instruction(inst).operands.len() {
            let operand = self.function.instruction(inst).operands[slot];
            if operand == old {
                replace_operand(self, inst, slot, new);
                self.worklist.add(inst);
                changed = true;
            } else {
                changed |= self.replace_in_instruction(operand, old, new, depth + 1);
            }
        }
        changed
    }
}

#[cfg(test)]
#[path = "combine_replace_in_tests.rs"]
mod tests;
