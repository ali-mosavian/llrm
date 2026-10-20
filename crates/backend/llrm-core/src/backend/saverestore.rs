//! Where a procedure's frame and kept registers are set up and taken back: one
//! Save block and one Restore block, from `ShrinkWrapImpl` in
//! llvm/lib/CodeGen/ShrinkWrap.cpp at d1106deb71cc.
//!
//! The pass reads the dominator and post-dominator trees, the cycles and the
//! block frequencies of the body, and what each block does with the frame
//! ([`Frame`]); it changes nothing. [`run`] answers with the [`Placement`] the
//! procedure's prologue and epilogue go to, or none where the entry is as good.
//!
//! # Example
//! A procedure that returns at once when its argument is 0 and otherwise calls
//! itself (`hanoi`). Only block 1 needs the frame, and it dominates and is
//! post-dominated by itself, so the procedure is set up and taken back there:
//!
//! ```text
//!   0: test; je 2          save: 1
//!   1: call hanoi          restore: 1
//!   2: ret                 (the entry and the return pay nothing)
//! ```
//!
//! This is `run` over that graph with block 1 as the only block that touches
//! the frame (`test_a_call_on_one_path_is_wrapped_there_and_not_at_the_entry`).
//!
//! DEVIATE 1: no EH pads, funclets or `INLINEASM_BR` targets: LIR has none, so
//! their arms are absent.
//! DEVIATE 2: `postShrinkWrapping` does not make the split block, it names it
//! ([`Placement::split`]); the consumer puts the epilogue on those edges.
//! LLVM then recalculates the trees only for its asserts, which are not kept.
//! DEVIATE 3: `RegScavenger`, `getCurrentCSRs` and the frame-index arms of
//! `useOrDefCSROrFI` are the caller's [`Frame::touches`]: what is a kept
//! register, a frame cell or the stack pointer is the target's to say.

use llrm_mir::blocklist::BlockList;
use llrm_mir::cycleinfo::{CycleInfo, FlowGraph};
use llrm_mir::domtree::{DomGraph, DomTreeBase};
use llrm_mir::module::BlockId;

/// What the pass asks of the target and the body (`TargetFrameLowering` and
/// `useOrDefCSROrFI`).
pub trait Frame {
    /// `useOrDefCSROrFI` over the instructions of `block`: one needs the
    /// prologue before it and the epilogue after it. `stack_address_used` is
    /// that a stack address was computed on a path to the block, which makes
    /// any access that may reach the stack count.
    fn touches(
        &self,
        block: BlockId,
        stack_address_used: bool,
    ) -> bool;

    /// `useOrDefCSROrFI` over the terminators of `block` only.
    fn terminators_touch(
        &self,
        block: BlockId,
    ) -> bool;

    /// `isReturnBlock`.
    fn is_return(
        &self,
        block: BlockId,
    ) -> bool;

    /// `canUseAsPrologue`.
    fn can_use_as_prologue(
        &self,
        _block: BlockId,
    ) -> bool {
        true
    }

    /// `canUseAsEpilogue`.
    fn can_use_as_epilogue(
        &self,
        _block: BlockId,
    ) -> bool {
        true
    }

    /// `isAnalyzableBB`: the block's branches can be rewritten.
    fn analyzable(
        &self,
        _block: BlockId,
    ) -> bool {
        true
    }
}

/// `MachineBlockFrequencyInfo` as the pass reads it.
pub trait Frequencies {
    fn entry(&self) -> u64;
    fn block(
        &self,
        block: BlockId,
    ) -> u64;
}

/// The prologue goes at the top of `save`, the epilogue before the terminators
/// of `restore`; or, with `split`, on a new block between each of those
/// predecessors of `restore` and `restore` (`tryToSplitRestore`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Placement {
    pub save: BlockId,
    pub restore: BlockId,
    pub split: Option<Vec<BlockId>>,
}

type Tree<const POST: bool> = DomTreeBase<POST>;

struct Shrink<'a, G: FlowGraph, F: Frame, Q: Frequencies> {
    graph: &'a G,
    frame: &'a F,
    freq: &'a Q,
    cycles: &'a CycleInfo,
    mdt: Tree<false>,
    mpdt: Tree<true>,
    save: Option<BlockId>,
    restore: Option<BlockId>,
    entry: BlockId,
    entry_freq: u64,
}

/// `ShrinkWrapImpl::run`
/// ([:945](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/ShrinkWrap.cpp#L945-L996)).
pub fn run<G: FlowGraph>(
    graph: &G,
    frame: &impl Frame,
    freq: &impl Frequencies,
) -> Option<Placement> {
    let entry = graph.entry()?;
    let cycles = CycleInfo::compute(graph);
    // An irreducible cycle may hold blocks `getLoopFor` does not report.
    if cycles.cycles().any(|one| !cycles.is_reducible(one)) {
        return None;
    }
    let (mut mdt, mut mpdt) = (Tree::<false>::new(), Tree::<true>::new());
    mdt.recalculate(graph);
    mpdt.recalculate(graph);
    let mut pass = Shrink {
        graph,
        frame,
        freq,
        cycles: &cycles,
        mdt,
        mpdt,
        save: None,
        restore: None,
        entry,
        entry_freq: freq.entry(),
    };
    let candidate = pass.perform_shrink_wrapping();
    let split = pass.post_shrink_wrapping(candidate);
    if !candidate && split.is_none() {
        return None;
    }
    if !pass.interesting() {
        return None;
    }
    Some(Placement { save: pass.save?, restore: pass.restore?, split })
}

impl<G: FlowGraph, F: Frame, Q: Frequencies> Shrink<'_, G, F, Q> {
    /// `ArePointsInteresting`.
    fn interesting(&self) -> bool {
        self.save.is_some_and(|save| save != self.entry) && self.restore.is_some()
    }

    fn in_loop(
        &self,
        block: BlockId,
    ) -> bool {
        self.cycles.get_cycle(block).is_valid()
    }

    fn reachable(
        &self,
        block: BlockId,
    ) -> bool {
        self.mdt.is_reachable_from_entry(block)
    }

    /// `FindIDom` over the dominator tree.
    fn idom(
        &self,
        block: BlockId,
        blocks: &[BlockId],
        strict: bool,
    ) -> Option<BlockId> {
        if blocks.is_empty() {
            return if strict { None } else { Some(block) };
        }
        let found = self.mdt.find_nearest_common_dominator_of(self.graph, blocks);
        if strict && found == Some(block) { None } else { found }
    }

    /// `FindIDom` over the post-dominator tree.
    fn ipdom(
        &self,
        block: BlockId,
        blocks: &[BlockId],
    ) -> Option<BlockId> {
        if blocks.is_empty() {
            return None;
        }
        let found = self.mpdt.find_nearest_common_dominator_of(self.graph, blocks);
        if found == Some(block) { None } else { found }
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> Vec<BlockId> {
        self.graph.predecessors(block).into_iter().collect()
    }

    fn successors(
        &self,
        block: BlockId,
    ) -> Vec<BlockId> {
        self.graph.successors(block).into_iter().collect()
    }

    /// `updateSaveRestorePoints`
    /// ([:693](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/ShrinkWrap.cpp#L693)):
    /// Save and Restore so that `block` is between them.
    fn update_save_restore_points(
        &mut self,
        block: BlockId,
    ) {
        self.save = match self.save {
            None => Some(block),
            Some(save) => self.mdt.find_nearest_common_dominator(self.graph, save, block),
        };
        assert!(self.save.is_some());
        self.restore = match self.restore {
            None => Some(block),
            // A block outside the post-dominator tree never returns.
            Some(restore) if self.mpdt.contains(block) => {
                self.mpdt.find_nearest_common_dominator(self.graph, restore, block)
            }
            Some(_) => None,
        };
        // The epilogue goes before the terminators: one that needs the frame
        // happens before the restore point.
        if self.restore == Some(block) && self.frame.terminators_touch(block) {
            let successors = self.successors(block);
            self.restore = if successors.is_empty() { None } else { self.ipdom(block, &successors) };
        }
        // Save dominates Restore, Restore post-dominates Save, and both are
        // outside any loop.
        while let (Some(save), Some(restore)) = (self.save, self.restore) {
            let dominates = self.mdt.dominates(save, restore);
            let post_dominates = self.mpdt.dominates(restore, save);
            if dominates && post_dominates && !self.in_loop(save) && !self.in_loop(restore) {
                break;
            }
            if !dominates {
                self.save = self.mdt.find_nearest_common_dominator(self.graph, save, restore);
                continue;
            }
            if !post_dominates {
                self.restore = self.mpdt.find_nearest_common_dominator(self.graph, restore, save);
            }
            let Some(restore) = self.restore else { break };
            if !(self.in_loop(save) || self.in_loop(restore)) {
                continue;
            }
            if self.cycles.get_cycle_depth(save) > self.cycles.get_cycle_depth(restore) {
                // Push Save out of its loop.
                let preds: Vec<BlockId> =
                    self.predecessors(save).into_iter().filter(|&one| self.reachable(one)).collect();
                self.save = self.idom(save, &preds, true);
                if self.save.is_none() {
                    break;
                }
            } else {
                // Push Restore out of its loop, past the post-dominator of its
                // exits.
                let cycle = self.cycles.get_cycle(restore);
                let mut ipdom = Some(restore);
                for &exiting in self.cycles.get_blocks(cycle) {
                    let leaves = self.successors(exiting).into_iter().any(|one| !self.cycles.contains(cycle, one));
                    if !leaves {
                        continue;
                    }
                    ipdom = ipdom.and_then(|at| self.ipdom(at, &self.successors(exiting)));
                    if ipdom.is_none() {
                        break;
                    }
                }
                match ipdom {
                    Some(at) if self.cycles.get_cycle_depth(at) < self.cycles.get_cycle_depth(restore) => {
                        self.restore = Some(at)
                    }
                    _ => {
                        self.restore = None;
                        break;
                    }
                }
            }
        }
    }

    /// `performShrinkWrapping`
    /// ([:807](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/ShrinkWrap.cpp#L807)).
    fn perform_shrink_wrapping(&mut self) -> bool {
        // Conservatively, every block may use a stack address; the blocks
        // walked in reverse post-order prove the opposite.
        let mut stack_address_used = vec![true; self.graph.max_number()];
        for block in reverse_post_order(self.graph) {
            let mut used = self.predecessors(block).iter().any(|one| stack_address_used[one.0 as usize]);
            if self.frame.touches(block, used) {
                self.update_save_restore_points(block);
                if !self.interesting() {
                    return false;
                }
                used = true;
            }
            stack_address_used[block.0 as usize] = used;
        }
        if !self.interesting() {
            // No block touched the frame or a kept register.
            assert!(self.save.is_none() && self.restore.is_none(), "We miss a shrink-wrap opportunity?!");
            return false;
        }
        loop {
            let (Some(save), Some(restore)) = (self.save, self.restore) else { break };
            let cheap = self.entry_freq >= self.freq.block(save);
            let can_prologue = self.frame.can_use_as_prologue(save);
            if cheap
                && self.entry_freq >= self.freq.block(restore)
                && can_prologue
                && self.frame.can_use_as_epilogue(restore)
            {
                break;
            }
            let moved;
            if !cheap || !can_prologue {
                self.save = self.idom(save, &self.predecessors(save), true);
                let Some(at) = self.save else { break };
                moved = at;
            } else {
                self.restore = self.ipdom(restore, &self.successors(restore));
                let Some(at) = self.restore else { break };
                moved = at;
            }
            self.update_save_restore_points(moved);
            if self.save.is_none() || self.restore.is_none() {
                break;
            }
        }
        self.interesting()
    }

    /// `postShrinkWrapping`
    /// ([:553](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/ShrinkWrap.cpp#L553)):
    /// the restore block split so that the paths that need nothing skip it.
    /// The predecessors that go through the new block, where it is made.
    fn post_shrink_wrapping(
        &mut self,
        has_candidate: bool,
    ) -> Option<Vec<BlockId>> {
        let (init_save, init_restore) = if has_candidate {
            (self.save?, self.restore?)
        } else {
            let mut returns = self.graph.nodes().iter().copied().filter(|&one| self.frame.is_return(one));
            let only = returns.next()?;
            // Several restore points are not supported.
            if returns.next().is_some() {
                return None;
            }
            (self.entry, only)
        };
        if init_restore == init_save
            || !self.mdt.dominates(init_save, init_restore)
            || !self.mpdt.dominates(init_restore, init_save)
        {
            return None;
        }
        let mut dirty = vec![false; self.graph.max_number()];
        for &block in self.graph.nodes() {
            if self.reachable(block) && self.frame.touches(block, true) {
                dirty[block.0 as usize] = true;
            }
        }
        let reached = self.reachable_by_dirty(&dirty);
        // `checkIfRestoreSplittable`.
        if self.frame.touches(init_restore, true) {
            return None;
        }
        let (mut dirty_preds, mut clean_preds) = (Vec::new(), Vec::new());
        for pred in self.predecessors(init_restore) {
            if !self.frame.analyzable(pred) {
                return None;
            }
            if reached[pred.0 as usize] { dirty_preds.push(pred) } else { clean_preds.push(pred) }
        }
        if dirty_preds.is_empty() || clean_preds.is_empty() {
            return None;
        }
        // The new save point dominates every dirty block.
        let mut new_save = self.idom(dirty_preds[0], &dirty_preds, false);
        while let Some(at) = new_save {
            let has_dirty_pred = self.predecessors(at).iter().any(|one| reached[one.0 as usize]);
            if !(has_dirty_pred || self.entry_freq < self.freq.block(at) || self.in_loop(at)) {
                break;
            }
            let preds: Vec<BlockId> = self.predecessors(at).into_iter().filter(|&one| self.reachable(one)).collect();
            if preds.is_empty() {
                break;
            }
            new_save = self.idom(preds[0], &preds, false);
        }
        let new_save = new_save?;
        if new_save == init_save
            || self.save_reachable_through_clean(new_save, &clean_preds)
            || !self.frame.can_use_as_prologue(new_save)
            || !self.frame.can_use_as_epilogue(init_restore)
        {
            return None;
        }
        self.save = Some(new_save);
        self.restore = Some(init_restore);
        Some(dirty_preds)
    }

    /// `collectBlocksReachableByDirty`: the dirty blocks and all they reach.
    fn reachable_by_dirty(
        &self,
        dirty: &[bool],
    ) -> Vec<bool> {
        let mut reached = vec![false; self.graph.max_number()];
        for &block in self.graph.nodes() {
            if !dirty[block.0 as usize] || reached[block.0 as usize] {
                continue;
            }
            // `markAllReachable`.
            reached[block.0 as usize] = true;
            let mut work = self.successors(block);
            while let Some(next) = work.pop() {
                if std::mem::replace(&mut reached[next.0 as usize], true) {
                    continue;
                }
                work.extend(self.successors(next));
            }
        }
        reached
    }

    /// `isSaveReachableThroughClean`.
    fn save_reachable_through_clean(
        &self,
        save: BlockId,
        clean_preds: &[BlockId],
    ) -> bool {
        let mut seen = vec![false; self.graph.max_number()];
        let mut work = clean_preds.to_vec();
        while let Some(block) = work.pop() {
            if block == save {
                return true;
            }
            if std::mem::replace(&mut seen[block.0 as usize], true) {
                continue;
            }
            work.extend(self.predecessors(block));
        }
        false
    }
}

/// The blocks reachable from the entry in reverse post-order
/// (`ReversePostOrderTraversal`).
fn reverse_post_order(graph: &impl DomGraph) -> Vec<BlockId> {
    let Some(entry) = graph.entry() else { return Vec::new() };
    let mut seen = vec![false; graph.max_number()];
    let mut order = Vec::new();
    let mut stack: Vec<(BlockId, BlockList, usize)> = vec![(entry, graph.successors(entry), 0)];
    seen[entry.0 as usize] = true;
    while let Some((block, successors, next)) = stack.last_mut() {
        if let Some(&to) = successors.get(*next) {
            *next += 1;
            if !std::mem::replace(&mut seen[to.0 as usize], true) {
                stack.push((to, graph.successors(to), 0));
            }
        } else {
            order.push(*block);
            stack.pop();
        }
    }
    order.reverse();
    order
}

#[cfg(test)]
#[path = "saverestore_tests.rs"]
mod tests;
