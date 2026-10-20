//! `formLCSSAForInstructions` (`LCSSA.cpp`): a loop's value read outside the
//! loop is read through a phi in each exit block that value dominates.
//!
//! # How it works
//! For each value on the worklist, the uses outside its loop (a phi's use is
//! at its incoming block) are collected. Each exit block the value dominates
//! gets a phi of it over the block's predecessors, and each use is rewritten
//! to the phi of its exit block, to the only phi, or through an
//! [`SsaUpdater`](crate::ssaupdater::SsaUpdater) when several phis exist.
//!
//! # Example
//! `%x` defined in loop `b1`, read in `b3` after its exit: `b3` gets
//! `%x.lcssa = phi [ %x, %b1 ]` and the read uses it.
//!
//! # Deviations
//! - No SCEV cache of the new phis (`SE->getSCEV(PN)` only fills a cache), no
//!   debug records, no lifetime markers: MIR has neither.
//!
//! # Used by
//! [`super::expand_iv`]'s `fixupLCSSAFormFor`.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Function, InstId, Operand, Use};
use llrm_mir::opcode::{Flags, Opcode};

use crate::occurrence::from_arms;
use crate::ssaupdater::SsaUpdater;

/// What `formLCSSAForInstructions` made.
#[derive(Default)]
pub struct Formed {
    /// `InsertedPHIs`.
    pub inserted: Vec<InstId>,
    /// `PHIsToRemove`: phis with no use rewritten.
    pub to_remove: Vec<InstId>,
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp#L82
/// `formLCSSAForInstructionsImpl`.
pub fn form_lcssa_for_instructions(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    loops: &LoopInfo,
    tree: &DomTree,
    mut worklist: Vec<InstId>,
) -> Formed {
    let mut formed = Formed::default();
    let mut local_to_remove: Vec<InstId> = Vec::new();
    while let Some(i) = worklist.pop() {
        let Some(value) = function.instruction(i).result else { continue };
        let inst_bb = function.parent(i).expect("a placed instruction");
        let Some(l) = loops.get_loop_for(inst_bb) else { continue };
        let exit_blocks = l.get_exit_blocks(function);
        if exit_blocks.is_empty() {
            continue;
        }
        let ty = function.value(value).ty;
        let mut uses_to_rewrite: Vec<Use> = Vec::new();
        for one in
            function.users(value).into_iter().map(|one| Use { user: one.user, index: one.index }).collect::<Vec<_>>()
        {
            let user_bb = function.parent(one.user).expect("a placed user");
            if !tree.is_reachable_from_entry(user_bb) {
                let poison = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison }));
                function.set_operand(one.user, one.index as usize, poison);
                continue;
            }
            let at = incoming_or_parent(function, one);
            if inst_bb != at && !l.contains(at) {
                uses_to_rewrite.push(one);
            }
        }
        if uses_to_rewrite.is_empty() {
            continue;
        }

        let name = function.value(value).name.clone();
        let mut ssa = SsaUpdater::new(ty, name.as_deref());
        let mut added: Vec<InstId> = Vec::new();
        let mut post_process: Vec<InstId> = Vec::new();
        for &exit_bb in &exit_blocks {
            if !tree.dominates(inst_bb, exit_bb) || ssa.has_value_for_block(exit_bb) {
                continue;
            }
            let phi = function.create_instruction(
                Opcode::Phi,
                ty,
                Vec::new(),
                Flags::default(),
                name.as_ref().map(|n| format!("{n}.lcssa")).as_deref(),
            );
            let first = function.block(exit_bb).first().expect("a terminator");
            function.insert(phi, Position::Before(first)).expect("a placed block");
            formed.inserted.push(phi);
            let preds = function.edge_predecessors(exit_bb);
            function
                .set_operands(phi, from_arms(&preds.iter().map(|&p| (Operand::Value(value), p)).collect::<Vec<_>>()));
            for (n, &pred) in preds.iter().enumerate() {
                if !l.contains(pred) {
                    uses_to_rewrite.push(Use { user: phi, index: 2 * n as u32 });
                }
            }
            added.push(phi);
            ssa.add_available_value(exit_bb, Operand::Value(function.instruction(phi).result.expect("a phi value")));
            if loops.get_loop_for(exit_bb).is_some_and(|other| !l.contains_loop(Some(other))) {
                post_process.push(phi);
            }
        }

        for one in uses_to_rewrite {
            let user_bb = incoming_or_parent(function, one);
            let front = function.block(user_bb).first().expect("a terminator");
            if function.instruction(front).opcode == Opcode::Phi && exit_blocks.contains(&user_bb) {
                let phi = function.instruction(front).result.expect("a phi value");
                function.set_operand(one.user, one.index as usize, Operand::Value(phi));
            } else if let [only] = added.as_slice() {
                let phi = function.instruction(*only).result.expect("a phi value");
                function.set_operand(one.user, one.index as usize, Operand::Value(phi));
            } else {
                ssa.rewrite_use(context, layout, function, one);
            }
        }

        for &phi in ssa.inserted_phis() {
            if let Some(other) = loops.get_loop_for(function.parent(phi).expect("placed"))
                && !l.contains_loop(Some(other))
            {
                post_process.push(phi);
            }
            formed.inserted.push(phi);
        }
        for phi in post_process {
            if !function.users(function.instruction(phi).result.expect("a phi value")).is_empty() {
                worklist.push(phi);
            }
        }
        for phi in added {
            let result = function.instruction(phi).result.expect("a phi value");
            if function.users(result).is_empty() && !local_to_remove.contains(&phi) {
                local_to_remove.push(phi);
            }
        }
    }
    formed.to_remove = local_to_remove;
    formed
}

/// The block a use is at: a phi's use is at its incoming block.
fn incoming_or_parent(
    function: &Function,
    one: Use,
) -> BlockId {
    let user = function.instruction(one.user);
    if user.opcode == Opcode::Phi
        && let Operand::Block(from) = user.operands[one.index as usize + 1]
    {
        return from;
    }
    function.parent(one.user).expect("a placed user")
}

#[cfg(test)]
#[path = "lcssa_form_tests.rs"]
mod tests;
