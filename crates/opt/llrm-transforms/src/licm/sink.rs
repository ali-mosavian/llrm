//! `sinkRegion` and what it asks (`LICM.cpp`): decisions only.

use llrm_analysis::cfg;
use llrm_analysis::mssa::Mssa;
use llrm_analysis::mssa::alias::AliasQuery;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Outer};

use super::hoist::can_sink_or_hoist_inst;
use super::{Flags, LoopCtx, LoopSafety, calls};

/// What `sinkRegion` does to an instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sinking {
    /// `isInstructionTriviallyDead`: erased in place of sunk.
    Erased,
    /// Cloned into the exit blocks its users stand in.
    Sunk,
}

/// [`collectChildrenInLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L486):
/// the loop's blocks from the header, a block before the blocks it immediately
/// dominates.
fn collect_children_in_loop(ctx: &LoopCtx) -> Vec<BlockId> {
    let mut worklist = vec![cfg::block(ctx.loop_.header)];
    let mut at = 0;
    while at < worklist.len() {
        let parent = cfg::id(worklist[at]);
        for &block in ctx.function.layout() {
            if ctx.contains_block(block) && ctx.shape.dominance.immediate(ctx.function, cfg::id(block)) == Some(parent)
            {
                worklist.push(block);
            }
        }
        at += 1;
    }
    worklist
}

/// `Instruction::mayHaveSideEffects`: `mayWriteToMemory() || mayThrow() ||
/// !willReturn()`.
fn may_have_side_effects(
    unit: &passes::Unit,
    outer: &Outer,
    accesses: &llrm_analysis::memoryssa::Accesses,
    inst: InstId,
) -> bool {
    let function = &*unit.function;
    match &function.instruction(inst).opcode {
        Opcode::Store { .. } => true,
        Opcode::Load { volatile, .. } => *volatile,
        Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Resume => {
            accesses.writes(inst).is_none_or(|writes| !writes.is_empty())
                || calls::may_throw(outer, unit.context, function, inst)
                || !calls::will_return(outer, unit.context, function, inst)
        }
        Opcode::Ret | Opcode::Br | Opcode::Switch | Opcode::Unreachable | Opcode::LandingPad { .. } => true,
        _ => false,
    }
}

/// The instructions that read `inst`'s result and are still in the function.
fn users(
    ctx: &LoopCtx,
    inst: InstId,
) -> Vec<InstId> {
    let Some(result) = ctx.function.instruction(inst).result else { return Vec::new() };
    let gone = ctx.state.gone.borrow();
    let mut found: Vec<InstId> =
        ctx.function.users(result).iter().map(|one| one.user).filter(|user| !gone.contains(user)).collect();
    found.dedup();
    found
}

/// [`sinkRegion`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L553):
/// the instructions it erases as dead and sinks, bottom-up. Not built: the
/// loop-nest mode, and `isFoldableInLoop` (a GEP the target folds into its
/// load or store stays in the loop: needs the target's cost, taken as never
/// foldable, which sinks more GEPs than LLVM).
#[allow(clippy::too_many_arguments)]
pub fn sink_region(
    unit: &passes::Unit,
    outer: &Outer,
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    accesses: &llrm_analysis::memoryssa::Accesses,
    flags: &mut Flags,
    safety: &LoopSafety,
) -> Vec<(InstId, Sinking)> {
    let function = &*unit.function;
    let mut found = Vec::new();
    // We want to visit children before parents.
    for block in collect_children_in_loop(ctx).into_iter().rev() {
        if ctx.in_sub_loop(block) {
            continue;
        }
        for &inst in function.block(block).instructions().iter().rev() {
            if ctx.state.gone.borrow().contains(&inst) {
                continue;
            }
            // The instruction is not used in the loop if it is dead. In this
            // case, we just delete it instead of sinking it.
            if function.instruction(inst).result.is_some()
                && users(ctx, inst).is_empty()
                && !may_have_side_effects(unit, outer, accesses, inst)
                && !function.instruction(inst).opcode.is_terminator()
            {
                ctx.state.gone.borrow_mut().insert(inst);
                found.push((inst, Sinking::Erased));
                continue;
            }
            // Check to see if we can sink this instruction to the exit blocks
            // of the loop. We can do this if the all users of the instruction
            // are outside of the loop.
            if !may_have_side_effects(unit, outer, accesses, inst)
                && let Some(into) = is_not_used_in_loop(ctx, inst)
                && can_sink_or_hoist_inst(unit, outer, ctx, mssa, aa, flags, safety, inst, true)
                && can_sink(ctx, inst)
            {
                ctx.state.gone.borrow_mut().insert(inst);
                ctx.state.moved.borrow_mut().insert(inst, into);
                found.push((inst, Sinking::Sunk));
            }
        }
    }
    found
}

/// [`isNotUsedOrFoldableInLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1106)
/// with nothing foldable: every user is outside the loop. The block of the
/// first user.
fn is_not_used_in_loop(
    ctx: &LoopCtx,
    inst: InstId,
) -> Option<BlockId> {
    let users = users(ctx, inst);
    if users.iter().any(|&user| ctx.contains(user)) {
        return None;
    }
    users.first().and_then(|&user| ctx.function.parent(user))
}

/// Whether
/// [`sink`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1353)
/// goes through: every user is an LCSSA phi that is trivially replaceable or
/// whose block can have its predecessors split. DEVIATE: a user that is not a
/// phi (the loop is not in LCSSA form) is not sunk, where LLVM asserts.
fn can_sink(
    ctx: &LoopCtx,
    inst: InstId,
) -> bool {
    let function = ctx.function;
    let Some(result) = function.instruction(inst).result else { return false };
    let users = users(ctx, inst);
    !users.is_empty()
        && users.iter().all(|&user| {
            let instruction = function.instruction(user);
            if !matches!(instruction.opcode, Opcode::Phi) {
                return false;
            }
            // `isTriviallyReplaceablePHI`: every incoming value is `inst`'s.
            let trivial = instruction.operands.chunks(2).all(|pair| pair[0] == Operand::Value(result));
            trivial || can_split_predecessors(ctx, user)
        })
}

/// `canSplitPredecessors`: the phi's block does not start with a landing pad.
fn can_split_predecessors(
    ctx: &LoopCtx,
    phi: InstId,
) -> bool {
    let function = ctx.function;
    let Some(block) = function.parent(phi) else { return false };
    let first = function
        .block(block)
        .instructions()
        .iter()
        .find(|&&one| !matches!(function.instruction(one).opcode, Opcode::Phi));
    !first.is_some_and(|&one| matches!(function.instruction(one).opcode, Opcode::LandingPad { .. }))
}
