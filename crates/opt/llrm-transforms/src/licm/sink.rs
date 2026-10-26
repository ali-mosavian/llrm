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
    /// `FoldableInLoop`: a GEP the target folds into its loads and stores in
    /// the loop; copied into the exit blocks, the original stays.
    Cloned,
}

#[cfg(test)]
thread_local! {
    /// The blocks `collect_children_in_loop` has looked at, for the test that
    /// holds its work to the loop's size.
    pub static CHILDREN_EXAMINED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// [`collectChildrenInLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L486):
/// the loop's blocks from the header, a block before the blocks it immediately
/// dominates.
fn collect_children_in_loop(ctx: &LoopCtx) -> Vec<BlockId> {
    let tree = ctx.shape.dominance.tree(ctx.function);
    let mut worklist = vec![cfg::block(ctx.loop_.header)];
    let mut at = 0;
    while at < worklist.len() {
        for child in tree.children(Some(worklist[at])) {
            #[cfg(test)]
            CHILDREN_EXAMINED.with(|one| one.set(one.get() + 1));
            if ctx.contains_block(child) {
                worklist.push(child);
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
            accesses
                .writes_in(
                    &llrm_analysis::memory::Unit::over(
                        unit.context,
                        unit.layout,
                        &outer.metadata,
                        &outer.globals,
                        function,
                    ),
                    inst,
                )
                .is_none_or(|writes| !writes.is_empty())
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
    let (gone, sunk) = (ctx.state.gone.borrow(), ctx.state.sunk.borrow());
    // An instruction sunk reads its operands from the exit block: a user there.
    let mut found: Vec<InstId> = ctx
        .function
        .users(result)
        .iter()
        .map(|one| one.user)
        .filter(|user| !gone.contains(user) || sunk.contains(user))
        .collect();
    found.dedup();
    found
}

/// [`sinkRegion`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L553):
/// the instructions it erases as dead and sinks, bottom-up. Not built: the
/// loop-nest mode.
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
        for inst in function.iter_block_rev(block) {
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
                && let Some((into, foldable)) = is_not_used_or_foldable_in_loop(unit, outer, ctx, inst)
                && can_sink_or_hoist_inst(unit, outer, ctx, mssa, aa, flags, safety, inst, true)
                && can_sink(ctx, inst)
            {
                if foldable {
                    found.push((inst, Sinking::Cloned));
                    continue;
                }
                ctx.state.gone.borrow_mut().insert(inst);
                ctx.state.sunk.borrow_mut().insert(inst);
                ctx.state.moved.borrow_mut().insert(inst, into);
                found.push((inst, Sinking::Sunk));
            }
        }
    }
    found
}

/// [`isFoldableInLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1086):
/// a GEP the target prices as free whose users in the loop are loads and
/// stores in its own block.
fn is_foldable_in_loop(
    unit: &passes::Unit,
    outer: &Outer,
    ctx: &LoopCtx,
    inst: InstId,
) -> bool {
    let function = &*unit.function;
    if !matches!(
        function.instruction(inst).opcode,
        Opcode::GetElementPtr { .. }
    ) {
        return false;
    }
    let tti = crate::looprotate::target_tti(outer.target());
    let price = llrm_mir::instructioncost::instruction_cost(
        &*tti,
        unit.context,
        unit.layout,
        &outer.globals,
        function,
        inst,
        llrm_mir::tti::CostKind::SizeAndLatency,
    );
    if !price.valid || price.value != 0 {
        return false;
    }
    let block = function.parent(inst);
    users(ctx, inst)
        .into_iter()
        .filter(|&user| ctx.contains(user))
        .all(
            |user| function.parent(user) == block
                && matches!(
                    function.instruction(user).opcode,
                    Opcode::Load { .. } | Opcode::Store { .. }
                ),
        )
}

/// [`isNotUsedOrFoldableInLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1106):
/// the block of the first user outside the loop, and whether users in the loop
/// remain, all of which fold `inst`.
fn is_not_used_or_foldable_in_loop(
    unit: &passes::Unit,
    outer: &Outer,
    ctx: &LoopCtx,
    inst: InstId,
) -> Option<(BlockId, bool)> {
    let users = users(ctx, inst);
    let in_loop = users.iter().any(|&user| ctx.contains(user));
    if in_loop && !is_foldable_in_loop(unit, outer, ctx, inst) {
        return None;
    }
    let first = users.iter().copied().find(|&user| !ctx.contains(user))?;
    let into = ctx.state.moved.borrow().get(&first).copied().or_else(|| ctx.function.parent(first))?;
    Some((into, in_loop))
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
    let users: Vec<InstId> = users(ctx, inst).into_iter().filter(|&user| !ctx.contains(user)).collect();
    !users.is_empty()
        && users.iter().all(|&user| {
            // A user that was sunk reads it through a phi made for the clone.
            if ctx.state.sunk.borrow().contains(&user) {
                return true;
            }
            let instruction = function.instruction(user);
            if !matches!(instruction.opcode, Opcode::Phi) {
                return false;
            }
            // `isTriviallyReplaceablePHI`: every incoming value is `inst`'s.
            let trivial = instruction.operands.chunks(2).all(|pair| pair[0] == Operand::Value(result));
            trivial || (!ctx.strict_sink && can_split_predecessors(ctx, user))
        })
}

/// `canSplitPredecessors`: the phi's block does not start with a landing pad.
fn can_split_predecessors(
    ctx: &LoopCtx,
    phi: InstId,
) -> bool {
    let function = ctx.function;
    let Some(block) = function.parent(phi) else { return false };
    let first = function.iter_block(block).find(|&one| !matches!(function.instruction(one).opcode, Opcode::Phi));
    !first.is_some_and(|one| matches!(function.instruction(one).opcode, Opcode::LandingPad { .. }))
}
