//! `visitCondBrInst`: the canonical forms of a conditional branch.
//!
//! Translation of
//! [`InstCombinerImpl::visitCondBrInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L4347)
//! at d1106deb71cc. `visit` has already asked `simplifyInstruction`.
//!
//! MIRROR: `br (not X), T, F` becomes `br X, F, T`, so that the guards SCEV
//! and the loop passes read are plain compares; a branch to one block on both
//! edges reads no condition.
//!
//! DEVIATE 1 (temporary): not made: the logical-and-with-invert form (MIR has
//! no logical `select` pattern here yet), the `fcmp` predicate canonical form
//! (`isCanonicalPredicate`), the undef and constant conditions (the driver
//! folds a branch on a number, `combine.rs`), and the replacement of the uses
//! of the condition dominated by an edge with true or false.
//!
//! DEVIATE 2: branch weights are not swapped (MIR carries none on a branch).
//!
//! # Example
//! ```text
//! %n = xor i1 %c, true                  br i1 %c, label %f, label %t
//! br i1 %n, label %t, label %f
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{query, replace_operand};

impl Combiner<'_> {
    /// `InstCombinerImpl::visitCondBrInst`.
    pub fn visit_cond_br(
        &mut self,
        inst: InstId,
    ) -> Visit {
        let branch = self.function.instruction(inst);
        let [cond, Operand::Block(taken), Operand::Block(other)] = branch.operands[..] else { return Visit::None };
        if branch.opcode != Opcode::Br || matches!(cond, Operand::Constant(_)) {
            return Visit::None;
        }
        // Change br (not X), label True, label False to: br X, label False, True
        if let Some(x) = pattern::not(&query(self, Some(inst)), cond)
            && !matches!(x, Operand::Constant(_))
        {
            self.function.set_operand(inst, 1, Operand::Block(other));
            self.function.set_operand(inst, 2, Operand::Block(taken));
            replace_operand(self, inst, 0, x);
            return Visit::Changed;
        }
        // If the condition is irrelevant, remove the use so that other
        // transforms on the condition become more effective.
        if taken == other {
            let false_ = self.int(self.type_of(cond), 0);
            replace_operand(self, inst, 0, false_);
            return Visit::Changed;
        }
        Visit::None
    }
}

#[cfg(test)]
#[path = "combine_branch_tests.rs"]
mod tests;
