//! `handlePotentiallyDeadSuccessors`: the blocks that a branch or switch on a
//! number leaves unreachable, found while the combine runs.
//!
//! Translation of
//! [`InstCombinerImpl::handlePotentiallyDeadSuccessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L4333),
//! `handlePotentiallyDeadBlocks` (:4320), `handleUnreachableFrom` (:4290) and
//! `addDeadEdge` (:4271) at d1106deb71cc, with `handleUnreachableTerminator`
//! (Local.cpp:2498).
//!
//! MIRROR: an edge that is never taken is recorded (`DeadEdges`) and the
//! inputs of its target's phis over it become poison. A block whose every way
//! in is dead (or comes from a block it dominates) loses its instructions
//! apart from the terminator, uses of them reading poison, and its own
//! successor edges are dead in turn. `prepare` does this for the branches on a
//! number that are there at the start; this is the same for those that
//! `simplifyInstruction` makes during the run.
//!
//! # Example
//! ```text
//! cut:  br i1 false, label %a, label %b       %p = phi i8 [ 5, %a ], [ 1, %b ]
//! ```
//! The phi's input over the edge to `%a` is poison, so the phi folds to `1`.
//!
//! # Used by
//! `visit_cond_br` and `visit_switch` (a condition that is a number).

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, defining, poison_phi_inputs};

impl Combiner<'_> {
    /// `handlePotentiallyDeadSuccessors`: `block` goes to `live` only (none for
    /// an undef or poison condition).
    pub(crate) fn handle_potentially_dead_successors(
        &mut self,
        block: BlockId,
        live: Option<BlockId>,
    ) {
        let mut worklist = Vec::new();
        for succ in self.function.successors(block).iter().copied().collect::<Vec<_>>() {
            if Some(succ) != live {
                self.add_dead_edge(block, succ, &mut worklist);
            }
        }
        self.handle_potentially_dead_blocks(&mut worklist);
    }

    /// `handlePotentiallyDeadBlocks`.
    fn handle_potentially_dead_blocks(
        &mut self,
        worklist: &mut Vec<BlockId>,
    ) {
        while let Some(block) = worklist.pop() {
            let tree = self.env.analyses.dominator_tree();
            let dead = self
                .function
                .predecessors(block)
                .iter()
                .all(|&pred| self.dead_edges.contains(&(pred, block)) || tree.dominates(block, pred));
            if dead {
                self.handle_unreachable_from(block, worklist);
            }
        }
    }

    /// `handleUnreachableFrom(&BB->front())`: every instruction of `block`
    /// but its terminator is erased, uses of it reading poison, and the
    /// terminator's operands that are instructions become poison.
    fn handle_unreachable_from(
        &mut self,
        block: BlockId,
        worklist: &mut Vec<BlockId>,
    ) {
        let last = self.function.terminator(block);
        let insts: Vec<InstId> = self.function.iter_block(block).collect();
        for &inst in insts.iter().rev() {
            if Some(inst) == last || matches!(self.function.instruction(inst).opcode, Opcode::LandingPad { .. }) {
                continue;
            }
            if let Some(result) = self.function.instruction(inst).result
                && !self.function.users(result).is_empty()
            {
                let ty = self.function.instruction(inst).ty;
                let poison = self.constant_of_kind(ty, ConstantKind::Poison);
                self.replace_inst_with(inst, poison);
            }
            self.erase(inst);
            self.made_ir_change = true;
        }
        if let Some(last) = last {
            let operands = self.function.instruction(last).operands.to_vec();
            for (at, operand) in operands.into_iter().enumerate() {
                if let Some(def) = defining(self.function, operand) {
                    let ty = self.type_of(operand);
                    let poison = self.constant_of_kind(ty, ConstantKind::Poison);
                    self.function.set_operand(last, at, poison);
                    self.add_to_worklist(def);
                    self.made_ir_change = true;
                }
            }
        }
        for succ in self.function.successors(block).iter().copied().collect::<Vec<_>>() {
            self.add_dead_edge(block, succ, worklist);
        }
    }

    /// `addDeadEdge`.
    fn add_dead_edge(
        &mut self,
        from: BlockId,
        to: BlockId,
        worklist: &mut Vec<BlockId>,
    ) {
        if !self.dead_edges.insert((from, to)) {
            return;
        }
        for phi in poison_phi_inputs(self.context, self.function, from, to) {
            self.add_to_worklist(phi);
            self.made_ir_change = true;
        }
        worklist.push(to);
    }

    fn constant_of_kind(
        &mut self,
        ty: llrm_mir::types::TypeId,
        kind: ConstantKind,
    ) -> Operand {
        Operand::Constant(self.context.constant(llrm_mir::context::Constant { ty, kind }))
    }
}
