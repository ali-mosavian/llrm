//! LoopSimplifyCFG: the branches and switches of a loop with a constant
//! condition become jumps, the blocks that leaves unreachable go, and a block
//! with one predecessor that jumps to it is merged into it.
//!
//! # How it works
//! A translation of LLVM's
//! [`LoopSimplifyCFG.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopSimplifyCFG.cpp)
//! `simplifyLoopCFG` (:693) at d1106deb71cc. MIRROR: `constantFoldTerminators`
//! works out, from the blocks in reverse postorder, which blocks and exits of
//! the loop stay reachable once every constant terminator is a jump
//! (`ConstantTerminatorFoldingImpl::analyze`); it gives up on an irreducible
//! loop, one entered indirectly, one with nothing to fold, one whose latch
//! would go, one with blocks that would stay but leave the loop, and one with
//! a dead exit and an LCSSA form broken by a token. Else the dead exits
//! keep their static reachability from a switch on `0` in the preheader
//! (`handleDeadExits`), the terminators are folded (`foldTerminators`), and
//! the dead blocks are emptied and deleted (`deleteDeadLoopBlocks`). Then
//! `mergeBlocksIntoPredecessors` merges the loop's blocks.
//!
//! DEVIATE 1: the slot runs the pass over every loop, innermost first, before
//! the next slot; the pin's loop pass manager runs LoopInstSimplify,
//! LoopSimplifyCFG, LICM and the rest on one loop, then the next.
//!
//! DEVIATE 2: no DomTreeUpdater, loop info or MemorySSA to update: the tree
//! and loops are made again after a loop changed. The LCSSA of the loops a
//! loop leaves (`formLCSSARecursively` on the outermost it left) is formed over
//! the function, after the edit and not before the folding.
//!
//! DEVIATE 3: no branch weights on the dummy switch (MIR has none).
//!
//! # Parity slot
//! `loop-simplifycfg@716`, after LoopInstSimplify in `LPM1`
//! ([PassBuilderPipelines.cpp#L716](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L716)).
//!
//! # Example
//! A loop whose body tests a constant `false` and leaves for an exit on true
//! loses the test: the exit's edge from the loop goes, the exit stays
//! reachable from a `switch i32 0` in the preheader until SimplifyCFG.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::{Loop, LoopInfo};
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::loopdeletion::loop_blocks_rpo;

/// `getOnlyLiveSuccessor`: the one successor a block with a constant
/// terminator can reach.
fn only_live_successor(
    context: &Context,
    function: &Function,
    bb: BlockId,
) -> Option<BlockId> {
    let term = function.terminator(bb)?;
    let instruction = function.instruction(term);
    let block = |operand: Operand| match operand {
        Operand::Block(block) => block,
        _ => unreachable!("a terminator's target is a block"),
    };
    let constant = |operand: Operand| match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    };
    match instruction.opcode {
        Opcode::Br if instruction.operands.len() == 3 => {
            let (first, second) = (block(instruction.operands[1]), block(instruction.operands[2]));
            if first == second {
                return Some(first);
            }
            let cond = constant(instruction.operands[0])?;
            Some(if cond == 0 { second } else { first })
        }
        Opcode::Switch => {
            let cond = constant(instruction.operands[0])?;
            let cases = instruction.operands[2..].chunks(2);
            for pair in cases {
                if constant(pair[0]) == Some(cond) {
                    return Some(block(pair[1]));
                }
            }
            Some(block(instruction.operands[1]))
        }
        _ => None,
    }
}

fn successor_list(
    function: &Function,
    bb: BlockId,
) -> Vec<BlockId> {
    function.successors(bb).into_iter().collect()
}

/// What `ConstantTerminatorFoldingImpl::analyze` finds.
struct Analysis {
    fold_candidates: Vec<BlockId>,
    dead_loop_blocks: Vec<BlockId>,
    live_exit_blocks: IdSet<BlockId>,
    dead_exit_blocks: Vec<BlockId>,
    blocks_in_loop_after_folding: IdSet<BlockId>,
    delete_current_loop: bool,
    has_irreducible_cfg: bool,
    has_indirect_entry: bool,
}

fn same_loop(
    loops: &LoopInfo,
    bb: BlockId,
    lp: Loop,
) -> bool {
    loops.get_loop_for(bb).is_some_and(|found| found.id() == lp.id())
}

/// `ConstantTerminatorFoldingImpl::analyze`.
fn analyze(
    context: &Context,
    function: &Function,
    loops: &LoopInfo,
    lp: Loop,
) -> Analysis {
    let mut found = Analysis {
        fold_candidates: Vec::new(),
        dead_loop_blocks: Vec::new(),
        live_exit_blocks: IdSet::default(),
        dead_exit_blocks: Vec::new(),
        blocks_in_loop_after_folding: IdSet::default(),
        delete_current_loop: false,
        has_irreducible_cfg: false,
        has_indirect_entry: false,
    };
    let rpo = loop_blocks_rpo(function, lp);

    // `hasIrreducibleCFG`: an edge from a later block to an earlier one that
    // is no backedge of a loop.
    let mut index = IdMap::<BlockId, usize>::new();
    for (at, &b) in rpo.iter().enumerate() {
        index.insert(b, at);
    }
    for &bb in &rpo {
        for succ in successor_list(function, bb) {
            if lp.contains(succ) && !loops.is_loop_header(succ) && index[&bb] > index[&succ] {
                found.has_irreducible_cfg = true;
                return found;
            }
        }
    }

    // A loop entered through an indirect branch has no preheader to split.
    if lp.get_loop_preheader(function).is_none() {
        found.has_indirect_entry = true;
        return found;
    }

    // Collect live and dead loop blocks and exits.
    let mut live_loop_blocks = IdSet::<BlockId>::default();
    live_loop_blocks.insert(lp.get_header());
    for &bb in &rpo {
        // A loop block not marked live so far is dead.
        if !live_loop_blocks.contains(&bb) {
            found.dead_loop_blocks.push(bb);
            continue;
        }
        let the_only_succ = only_live_successor(context, function, bb);
        // A block with one live successor is a candidate for folding; the
        // branches of child loops are folded when those are processed.
        let take_fold_candidate = the_only_succ.is_some() && same_loop(loops, bb, lp);
        if take_fold_candidate {
            found.fold_candidates.push(bb);
        }
        for succ in successor_list(function, bb) {
            if !take_fold_candidate || the_only_succ == Some(succ) {
                if lp.contains(succ) {
                    live_loop_blocks.insert(succ);
                } else {
                    found.live_exit_blocks.insert(succ);
                }
            }
        }
    }

    // All exit blocks not marked live are dead, if all their predecessors are
    // in the loop.
    let mut unique_dead_exits = IdSet::<BlockId>::default();
    for exit in lp.get_exit_blocks(function) {
        if !found.live_exit_blocks.contains(&exit)
            && unique_dead_exits.insert(exit)
            && function.predecessors(exit).into_iter().all(|pred| lp.contains(pred))
        {
            found.dead_exit_blocks.push(exit);
        }
    }

    // Whether the edge from->to stays after the folding.
    let is_edge_live = |from: BlockId, to: BlockId| {
        if !live_loop_blocks.contains(&from) {
            return false;
        }
        let the_only_succ = only_live_successor(context, function, from);
        the_only_succ.is_none() || the_only_succ == Some(to) || !same_loop(loops, from, lp)
    };

    // The loop is not destroyed if its latch is live.
    let latch = lp.get_loop_latch(function).expect("a single latch");
    found.delete_current_loop = !is_edge_live(latch, lp.get_header());
    if found.delete_current_loop {
        return found;
    }

    // The blocks that stay in the loop: a block with a live edge to one that
    // does; the latch does.
    found.blocks_in_loop_after_folding.insert(latch);
    for &bb in rpo.iter().rev() {
        let in_loop = successor_list(function, bb)
            .into_iter()
            .any(|succ| found.blocks_in_loop_after_folding.contains(&succ) && is_edge_live(bb, succ));
        if in_loop {
            found.blocks_in_loop_after_folding.insert(bb);
        }
    }
    found
}

fn poison(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Operand {
    let ty = function.instruction(inst).ty;
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison }))
}

/// `handleDeadExits`: the dead exits stay reachable from a `switch i32 0` in
/// the preheader, which sends the one live way to the split preheader.
fn handle_dead_exits(
    context: &Context,
    function: &mut Function,
    preheader: BlockId,
    dead_exits: &[BlockId],
) {
    if dead_exits.is_empty() {
        return;
    }
    let term = function.terminator(preheader).expect("a terminated preheader");
    let new_preheader = crate::edges::split_block_at(function, preheader, term);
    // `SplitBlock` puts the new block right after the old.
    function.move_block_after(new_preheader, preheader).expect("placed blocks");
    let jump = function.terminator(preheader).expect("the split's jump");
    let void = function.instruction(jump).ty;
    let i32_ty = context.types.int(32);
    let mut operands = vec![Operand::Constant(context.int(i32_ty, 0)), Operand::Block(new_preheader)];
    for (at, &exit) in dead_exits.iter().enumerate() {
        // Eliminate all phis and landing pads from dead exits.
        let mut dead: Vec<InstId> = crate::edges::phis(function, exit);
        if let Some(first) = function.iter_block(exit).find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
            && matches!(function.instruction(first).opcode, Opcode::LandingPad { .. })
        {
            dead.push(first);
        }
        for inst in dead {
            if let Some(result) = function.instruction(inst).result {
                let replacement = poison(context, function, inst);
                function.replace_value(result, replacement);
            }
            function.erase(inst).expect("a replaced instruction");
        }
        operands.push(Operand::Constant(context.int(i32_ty, at as i128 + 1)));
        operands.push(Operand::Block(exit));
    }
    let switch = function.create_instruction(Opcode::Switch, void, operands, Flags::default(), None);
    function.insert(switch, llrm_mir::edit::Position::Before(jump)).expect("a placed block");
    function.erase(jump).expect("the jump had no users");
}

/// `foldTerminators`.
fn fold_terminators(
    context: &Context,
    function: &mut Function,
    lp: Loop,
    candidates: &[BlockId],
) {
    for &bb in candidates {
        let the_only_succ = only_live_successor(context, function, bb).expect("one live successor");
        // Remove all successors but the live one.
        let mut dead_successors = IdSet::<BlockId>::default();
        let mut the_only_succ_duplicates = 0;
        for succ in successor_list(function, bb) {
            if succ != the_only_succ {
                dead_successors.insert(succ);
                // A successor in another loop keeps its one-input phi: it is
                // an LCSSA phi.
                function.predecessor_removed(context, succ, bb, !lp.contains(succ));
            } else {
                the_only_succ_duplicates += 1;
            }
        }
        // If the live successor was named more than once, it is named once
        // now: remove the redundant inputs of its phis.
        for _ in 1..the_only_succ_duplicates {
            function.predecessor_removed(context, the_only_succ, bb, !lp.contains(the_only_succ));
        }
        let term = function.terminator(bb).expect("a terminated block");
        let void = function.instruction(term).ty;
        let jump =
            function.create_instruction(Opcode::Br, void, vec![Operand::Block(the_only_succ)], Flags::default(), None);
        function.insert(jump, llrm_mir::edit::Position::Before(term)).expect("a placed block");
        function.erase(term).expect("the terminator had no users");
    }
}

/// `deleteDeadLoopBlocks` with `detachDeadBlocks(.., KeepOneInputPHIs)`.
fn delete_dead_loop_blocks(
    context: &Context,
    function: &mut Function,
    dead: &[BlockId],
) {
    let mut insts = Vec::new();
    for &bb in dead {
        // Every successor learns that one of its predecessors goes.
        for succ in successor_list(function, bb) {
            function.predecessor_removed(context, succ, bb, true);
        }
        // Zap the instructions; what reads one is replaced by poison, control
        // flow cannot get here.
        for inst in function.iter_block(bb).collect::<Vec<_>>().into_iter().rev() {
            if let Some(result) = function.instruction(inst).result
                && !function.users(result).is_empty()
            {
                let replacement = poison(context, function, inst);
                function.replace_value(result, replacement);
            }
            insts.push(inst);
        }
    }
    function.erase_all(&insts).expect("the dead blocks' instructions");
    // A phi of a block outside still names a dead block it keeps one input
    // for: those were dropped by `predecessor_removed`.
    function.erase_blocks(dead).expect("empty dead blocks");
}

/// `Loop::isLCSSAForm(DT, IgnoreTokens = false)`: every value defined in the
/// loop is read outside it by a phi alone, through an edge from inside.
fn is_lcssa_form(
    function: &Function,
    lp: Loop,
) -> bool {
    for &block in lp.get_blocks() {
        for inst in function.iter_block(block) {
            let Some(result) = function.instruction(inst).result else { continue };
            for used in function.users(result) {
                let Some(user_block) = function.parent(used.user) else { continue };
                let reads_from = if function.instruction(used.user).opcode == Opcode::Phi {
                    // The block the use comes from, as a phi names it.
                    match function.instruction(used.user).operands.get(used.index as usize + 1) {
                        Some(Operand::Block(from)) => *from,
                        _ => user_block,
                    }
                } else {
                    user_block
                };
                if !lp.contains(reads_from) {
                    return false;
                }
            }
        }
    }
    true
}

/// `constantFoldTerminators`: whether the loop changed.
fn constant_fold_terminators(
    context: &Context,
    function: &mut Function,
    loops: &LoopInfo,
    lp: Loop,
) -> bool {
    // Only loops with a single latch.
    if lp.get_loop_latch(function).is_none() {
        return false;
    }
    let found = analyze(context, function, loops, lp);
    if found.has_irreducible_cfg || found.has_indirect_entry {
        return false;
    }
    // Nothing to constant-fold.
    if found.fold_candidates.is_empty() {
        return false;
    }
    // TODO in LLVM: support deletion of the current loop.
    if found.delete_current_loop {
        return false;
    }
    // TODO in LLVM: blocks that are not dead but stop being part of the loop.
    if found.blocks_in_loop_after_folding.iter().count() + found.dead_loop_blocks.len() != lp.get_num_blocks() {
        return false;
    }
    // Tokens may breach LCSSA form, and the dead exit transform needs it
    // kept for every value.
    let preheader = lp.get_loop_preheader(function).expect("a preheader");
    if !found.dead_exit_blocks.is_empty() && !is_lcssa_form(function, lp) {
        return false;
    }

    handle_dead_exits(context, function, preheader, &found.dead_exit_blocks);
    fold_terminators(context, function, lp, &found.fold_candidates);
    if !found.dead_loop_blocks.is_empty() {
        delete_dead_loop_blocks(context, function, &found.dead_loop_blocks);
    }
    true
}

/// `mergeBlocksIntoPredecessors`: whether the loop changed.
fn merge_blocks_into_predecessors(
    context: &Context,
    function: &mut Function,
    loops: &LoopInfo,
    lp: Loop,
) -> bool {
    let mut changed = false;
    let mut merged = Vec::new();
    // The blocks are copied: they go as they merge.
    for succ in lp.get_blocks().to_vec() {
        if merged.contains(&succ) {
            continue;
        }
        let predecessors = function.predecessors(succ);
        let [pred] = predecessors[..] else { continue };
        let successors = function.successors(pred);
        if successors.len() != 1 || !same_loop(loops, pred, lp) {
            continue;
        }
        if crate::cfg::merge_block_into_predecessor(context, function, succ, false) {
            merged.push(succ);
            changed = true;
        }
    }
    function.erase_blocks(&merged).expect("merged blocks are empty");
    changed
}

fn loops_of(function: &Function) -> LoopInfo {
    let tree = DomTree::of(function);
    LoopInfo::analyze(function, || &tree)
}

/// `simplifyLoopCFG`: whether the loop changed. `loops` are the function's as
/// it stands.
fn simplify_loop_cfg(
    context: &Context,
    function: &mut Function,
    loops: &LoopInfo,
    header: BlockId,
) -> bool {
    let lp = loops.get_loop_for(header).expect("a loop headed by this block");
    // Constant-fold terminators with known constant conditions.
    let mut changed = constant_fold_terminators(context, function, loops, lp);
    // The folding changed the blocks and edges: the merging reads them again.
    let after;
    let loops_after = if changed {
        after = loops_of(function);
        &after
    } else {
        loops
    };
    // Eliminate unconditional branches by merging blocks into their
    // predecessors.
    if let Some(lp) = loops_after.get_loop_for(header).filter(|found| found.get_header() == header) {
        changed |= merge_blocks_into_predecessors(context, function, loops_after, lp);
    }
    changed
}

/// The pass over every loop of `function`, innermost first. Whether anything
/// changed.
pub fn loop_simplify_cfg(
    context: &Context,
    function: &mut Function,
) -> bool {
    let mut loops = loops_of(function);
    let headers: Vec<BlockId> =
        loops.get_loops_in_reverse_sibling_preorder().iter().rev().map(|one| one.get_header()).collect();
    let mut changed = false;
    // The loops are made again only after a loop changed the function.
    let mut stale = false;
    for header in headers {
        if stale {
            loops = loops_of(function);
            stale = false;
        }
        if !function.is_placed(header) || !loops.is_loop_header(header) {
            continue;
        }
        if simplify_loop_cfg(context, function, &loops, header) {
            changed = true;
            stale = true;
        }
    }
    if changed {
        // The loops a loop left need LCSSA phis for what the loop reads of
        // them.
        let _ = crate::lcssa::closed(function);
    }
    changed
}

pub struct LoopSimplifyCfg;

impl FunctionPass for LoopSimplifyCfg {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "loop-simplifycfg"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("loop-simplifycfg")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        _analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if loop_simplify_cfg(&*unit.context, unit.function) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "loopsimplifycfg_tests.rs"]
mod tests;
