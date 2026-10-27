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
//! MIRROR: a branch that reaches the end of the visit is registered with the
//! `DomConditionCache` (`DC.registerBranch`), so a query sees the branches
//! visited before it and no others.
//!
//! DEVIATE 1 (temporary): not made: the logical-and-with-invert form (MIR has
//! no logical `select` pattern here yet), the `fcmp` predicate canonical form
//! (`isCanonicalPredicate`), and the constant condition at visit time (the
//! driver folds a branch on a number, and `combine::prepare` kills the edges
//! of an undef or poison one, `combine.rs`).
//!
//! MIRROR: every use of the condition that an edge dominates becomes `true`
//! (the first edge) or `false` (the second): a second branch on the same
//! compare, further down, reads a number.
//!
//! DEVIATE 2: branch weights are not swapped (MIR carries none on a branch).
//!
//! # Example
//! ```text
//! %n = xor i1 %c, true                  br i1 %c, label %f, label %t
//! br i1 %n, label %t, label %f
//! ```
//!
//! # Used by
//! `visit` in [`combine`](crate::combine), which dispatches a `br` here
//! (`visit_cond_br`).

use llrm_mir::module::{InstId, Operand, Use};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::pattern;
use llrm_mir::valuetracking::range::edge_dominates_use;

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
        // Change br (not X), label True, label False to: br X, label False,
        // True
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
        // Replace all dominated uses of the condition with true/false.
        if let Operand::Value(value) = cond {
            let block = self.function.parent(inst).expect("a placed branch");
            let tree = self.env.analyses.dominator_tree();
            let ty = self.type_of(cond);
            let mut changed = false;
            let uses: Vec<Use> =
                self.function.users(value).into_iter().map(|one| Use { user: one.user, index: one.index }).collect();
            for used in uses {
                let function = &*self.function;
                let way = if edge_dominates_use(function, &tree, block, taken, &used) {
                    1
                } else if edge_dominates_use(function, &tree, block, other, &used) {
                    0
                } else {
                    continue;
                };
                let number = self.int(ty, way);
                replace_operand(self, used.user, used.index as usize, number);
                self.add_to_worklist(used.user);
                changed = true;
            }
            if changed {
                return Visit::Changed;
            }
        }
        // `DC.registerBranch(&BI)`.
        if let Some(conditions) = self.env.analyses.dom_conditions() {
            conditions.register_branch(&query(self, Some(inst)), inst);
        }
        Visit::None
    }
}

#[cfg(test)]
#[path = "combine_branch_tests.rs"]
mod tests;
