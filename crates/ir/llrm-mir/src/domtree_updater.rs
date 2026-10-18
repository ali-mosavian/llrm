//! `DomTreeUpdater`: edge updates to a dominator tree and a post-dominator
//! tree, applied at once (Eager) or queued until a reader asks for a tree
//! (Lazy).
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `GenericDomTreeUpdater.h`,
//! `GenericDomTreeUpdaterImpl.h`, `DomTreeUpdater.h` and `DomTreeUpdater.cpp`,
//! each function linking the lines it translates. A pass that edits the CFG
//! many times between two questions to the trees holds a Lazy updater: the
//! updates queue, and [`DomTreeUpdater::get_dom_tree`] applies what is
//! pending as one batch, which `applyUpdates` may turn into one rebuild.
//!
//! DEVIATE (anything MIR or Rust lacks):
//! 1. The trees are owned (`Option<DomTree>`), not pointed to: a Rust value
//!    cannot be shared with its owner while the updater edits it.
//!    [`DomTreeUpdater::into_trees`] gives them back.
//! 2. The graph is an argument of each call that reads the CFG, as in
//!    [`crate::domtree`].
//! 3. `deleteBB`, `callbackDeleteBB` and `validateDeleteBB` do not erase the
//!    block from the function (a MIR `Function` is edited by the pass, which
//!    erases a block with `erase_block` once nothing names it): the deferral of
//!    the trees' `eraseNode` until every pending update is applied is kept.
//!
//! # Example
//! A Lazy updater told of ten edge updates applies them as one batch when
//! the dominator tree is asked for.
//!
//! # Used by
//! The `Dominance` of `llrm-analysis`'s `cfg::Shape` (`rotate`).

use crate::dense::IdSet;
use crate::domtree::{DomGraph, DomTree, PostDomTree, Update, UpdateKind};
use crate::hash::HashSet;
use crate::module::BlockId;

/// `GenericDomTreeUpdater::UpdateStrategy`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdater.h#L33>
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateStrategy {
    Eager,
    Lazy,
}

/// `GenericDomTreeUpdater::CriticalEdge`: the blocks of a split critical edge.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdater.h#L192>
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CriticalEdge {
    pub from: BlockId,
    pub to: BlockId,
    pub new: BlockId,
}

/// `GenericDomTreeUpdater::DomTreeUpdate`.
#[derive(Clone, Copy, Debug)]
enum Pending {
    Update(Update),
    Split(CriticalEdge),
}

/// `GenericDomTreeUpdater` and `DomTreeUpdater`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdater.h#L21>
#[derive(Debug)]
pub struct DomTreeUpdater {
    pend_updates: Vec<Pending>,
    pend_dt_update_index: usize,
    pend_pdt_update_index: usize,
    dt: Option<DomTree>,
    pdt: Option<PostDomTree>,
    strategy: UpdateStrategy,
    deleted_bbs: IdSet<BlockId>,
    is_recalculating_dom_tree: bool,
    is_recalculating_post_dom_tree: bool,
}

impl DomTreeUpdater {
    /// The constructors: a tree, a post-dominator tree, both or none.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdater.h#L37>
    pub fn new(
        dt: Option<DomTree>,
        pdt: Option<PostDomTree>,
        strategy: UpdateStrategy,
    ) -> Self {
        Self {
            pend_updates: Vec::new(),
            pend_dt_update_index: 0,
            pend_pdt_update_index: 0,
            dt,
            pdt,
            strategy,
            deleted_bbs: IdSet::new(),
            is_recalculating_dom_tree: false,
            is_recalculating_post_dom_tree: false,
        }
    }

    /// The trees, with the updates pending applied (`~DomTreeUpdater` flushes).
    pub fn into_trees(
        mut self,
        graph: &impl DomGraph,
    ) -> (Option<DomTree>, Option<PostDomTree>) {
        self.flush(graph);
        (self.dt.take(), self.pdt.take())
    }

    pub fn is_lazy(&self) -> bool {
        self.strategy == UpdateStrategy::Lazy
    }

    pub fn is_eager(&self) -> bool {
        self.strategy == UpdateStrategy::Eager
    }

    pub fn has_dom_tree(&self) -> bool {
        self.dt.is_some()
    }

    pub fn has_post_dom_tree(&self) -> bool {
        self.pdt.is_some()
    }

    /// Whether a block is awaiting deletion: the deletion happens at a flush
    /// event, when all available trees are up to date.
    pub fn has_pending_deleted_bb(&self) -> bool {
        !self.deleted_bbs.is_empty()
    }

    pub fn is_bb_pending_deletion(
        &self,
        del_bb: BlockId,
    ) -> bool {
        if self.strategy == UpdateStrategy::Eager || self.deleted_bbs.is_empty() {
            return false;
        }
        self.deleted_bbs.contains(&del_bb)
    }

    pub fn has_pending_updates(&self) -> bool {
        self.has_pending_dom_tree_updates() || self.has_pending_post_dom_tree_updates()
    }

    pub fn has_pending_dom_tree_updates(&self) -> bool {
        self.dt.is_some() && self.pend_updates.len() != self.pend_dt_update_index
    }

    pub fn has_pending_post_dom_tree_updates(&self) -> bool {
        self.pdt.is_some() && self.pend_updates.len() != self.pend_pdt_update_index
    }

    /// `recalculate`: the entry block was replaced. Recalculates all available
    /// trees and flushes the blocks awaiting deletion.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L28>
    pub fn recalculate(
        &mut self,
        graph: &impl DomGraph,
    ) {
        if self.strategy == UpdateStrategy::Eager {
            if let Some(dt) = &mut self.dt {
                dt.recalculate(graph);
            }
            if let Some(pdt) = &mut self.pdt {
                pdt.recalculate(graph);
            }
            return;
        }
        // There is little performance gain if we pend the recalculation under
        // Lazy UpdateStrategy so we recalculate available trees immediately.

        // Prevent forceFlushDeletedBB() from erasing DomTree or PostDomTree
        // nodes.
        self.is_recalculating_dom_tree = true;
        self.is_recalculating_post_dom_tree = true;
        // Because all trees are going to be up-to-date after recalculation,
        // flush awaiting deleted BasicBlocks.
        self.force_flush_deleted_bb();
        if let Some(dt) = &mut self.dt {
            dt.recalculate(graph);
        }
        if let Some(pdt) = &mut self.pdt {
            pdt.recalculate(graph);
        }
        // Resume forceFlushDeletedBB() to erase DomTree or PostDomTree nodes.
        self.is_recalculating_dom_tree = false;
        self.is_recalculating_post_dom_tree = false;
        self.pend_dt_update_index = self.pend_updates.len();
        self.pend_pdt_update_index = self.pend_updates.len();
        self.drop_out_of_date_updates();
    }

    /// `applyUpdates`: submit updates to all available trees. Eager applies
    /// them at once, Lazy queues them. The CFG must already show them.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L59>
    pub fn apply_updates(
        &mut self,
        graph: &impl DomGraph,
        updates: &[Update],
    ) {
        if self.dt.is_none() && self.pdt.is_none() {
            return;
        }
        if self.strategy == UpdateStrategy::Lazy {
            self.pend_updates.reserve(updates.len());
            for &update in updates {
                if !Self::is_self_dominance(update) {
                    self.pend_updates.push(Pending::Update(update));
                }
            }
            return;
        }
        if let Some(dt) = &mut self.dt {
            dt.apply_updates(graph, updates);
        }
        if let Some(pdt) = &mut self.pdt {
            pdt.apply_updates(graph, updates);
        }
    }

    /// `applyUpdatesPermissive`: as `apply_updates`, discarding duplicated
    /// updates and the ones the CFG does not show (the deletion of an edge
    /// that still exists, the insertion of one that does not).
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L77>
    pub fn apply_updates_permissive(
        &mut self,
        graph: &impl DomGraph,
        updates: &[Update],
    ) {
        if self.dt.is_none() && self.pdt.is_none() {
            return;
        }
        let mut seen: HashSet<(BlockId, BlockId)> = HashSet::default();
        let mut deduplicated_updates: Vec<Update> = Vec::new();
        for &update in updates {
            let edge = (update.from, update.to);
            // Because it is illegal to submit updates that have already been
            // applied and updates to an edge need to be strictly ordered, it
            // is safe to infer the existence of an edge from the first update
            // to this edge: the first update to an edge is Delete if it
            // existed before, Insert if it did not. Later updates to it are
            // ignored and the current CFG inspected: if the edge still shows
            // as the first update implies, both happened and cancelled.
            if !Self::is_self_dominance(update) && seen.insert(edge) {
                // If the update doesn't appear in the CFG, it means that
                // either the change isn't made or relevant operations result
                // in a no-op.
                if Self::is_update_valid(graph, update) {
                    if self.is_lazy() {
                        self.pend_updates.push(Pending::Update(update));
                    } else {
                        deduplicated_updates.push(update);
                    }
                }
            }
        }
        if self.strategy == UpdateStrategy::Lazy {
            return;
        }
        if let Some(dt) = &mut self.dt {
            dt.apply_updates(graph, &deduplicated_updates);
        }
        if let Some(pdt) = &mut self.pdt {
            pdt.apply_updates(graph, &deduplicated_updates);
        }
    }

    /// `splitCriticalEdge`: the critical edge (`from`, `to`) was split with
    /// `new`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L129>
    pub fn split_critical_edge(
        &mut self,
        graph: &impl DomGraph,
        from: BlockId,
        to: BlockId,
        new: BlockId,
    ) {
        if self.dt.is_none() && self.pdt.is_none() {
            return;
        }
        let edge = CriticalEdge { from, to, new };
        if self.strategy == UpdateStrategy::Lazy {
            self.pend_updates.push(Pending::Split(edge));
            return;
        }
        if self.dt.is_some() {
            self.split_dt_critical_edges(graph, &[edge]);
        }
        if self.pdt.is_some() {
            self.split_pdt_critical_edges(graph, &[edge]);
        }
    }

    /// `getDomTree`: flushes the dominator tree's updates and returns it.
    /// Flushes the deleted blocks if both trees are up to date.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L151>
    pub fn get_dom_tree(
        &mut self,
        graph: &impl DomGraph,
    ) -> &DomTree {
        assert!(self.dt.is_some(), "Invalid acquisition of a null DomTree");
        self.apply_dom_tree_updates(graph);
        self.drop_out_of_date_updates();
        self.dt.as_ref().expect("asserted above")
    }

    /// `getPostDomTree`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L161>
    pub fn get_post_dom_tree(
        &mut self,
        graph: &impl DomGraph,
    ) -> &PostDomTree {
        assert!(self.pdt.is_some(), "Invalid acquisition of a null PostDomTree");
        self.apply_post_dom_tree_updates(graph);
        self.drop_out_of_date_updates();
        self.pdt.as_ref().expect("asserted above")
    }

    /// `flush`: applies all pending updates to the available trees and
    /// flushes the blocks awaiting deletion.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdater.h#L162>
    pub fn flush(
        &mut self,
        graph: &impl DomGraph,
    ) {
        self.apply_dom_tree_updates(graph);
        self.apply_post_dom_tree_updates(graph);
        self.drop_out_of_date_updates();
    }

    /// `isSelfDominance`: an update of a self loop does not affect either
    /// tree.
    fn is_self_dominance(update: Update) -> bool {
        update.from == update.to
    }

    fn apply_dom_tree_updates(
        &mut self,
        graph: &impl DomGraph,
    ) {
        self.apply_updates_impl::<true>(graph);
    }

    fn apply_post_dom_tree_updates(
        &mut self,
        graph: &impl DomGraph,
    ) {
        self.apply_updates_impl::<false>(graph);
    }

    /// `applyUpdatesImpl`: applies the updates a tree has not seen, a run of
    /// plain updates as one batch and a run of split edges together.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L276>
    fn apply_updates_impl<const IS_FORWARD: bool>(
        &mut self,
        graph: &impl DomGraph,
    ) {
        let has_tree = if IS_FORWARD { self.dt.is_some() } else { self.pdt.is_some() };
        // No pending DomTreeUpdates.
        if self.strategy != UpdateStrategy::Lazy || !has_tree {
            return;
        }
        // Only apply updates not are applied by (Post)DomTree.
        while if IS_FORWARD { self.has_pending_dom_tree_updates() } else { self.has_pending_post_dom_tree_updates() } {
            let at = if IS_FORWARD { self.pend_dt_update_index } else { self.pend_pdt_update_index };
            debug_assert!(at < self.pend_updates.len(), "there should be DomTree updates");
            let advanced;
            if let Pending::Update(_) = self.pend_updates[at] {
                let normal_updates: Vec<Update> = self.pend_updates[at..]
                    .iter()
                    .map_while(|one| if let Pending::Update(update) = one { Some(*update) } else { None })
                    .collect();
                if IS_FORWARD {
                    self.dt.as_mut().expect("checked").apply_updates(graph, &normal_updates);
                } else {
                    self.pdt.as_mut().expect("checked").apply_updates(graph, &normal_updates);
                }
                advanced = normal_updates.len();
            } else {
                let critical_edges: Vec<CriticalEdge> = self.pend_updates[at..]
                    .iter()
                    .map_while(|one| if let Pending::Split(edge) = one { Some(*edge) } else { None })
                    .collect();
                if IS_FORWARD {
                    self.split_dt_critical_edges(graph, &critical_edges);
                } else {
                    self.split_pdt_critical_edges(graph, &critical_edges);
                }
                advanced = critical_edges.len();
            }
            if IS_FORWARD {
                self.pend_dt_update_index += advanced;
            } else {
                self.pend_pdt_update_index += advanced;
            }
        }
    }

    /// `isUpdateValid`: whether the update appears in the CFG. It must be
    /// asked after the terminator of `from` is altered.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L304>
    fn is_update_valid(
        graph: &impl DomGraph,
        update: Update,
    ) -> bool {
        // Discard updates by inspecting the current state of successors of
        // From.
        let has_edge = graph.successors(update.from).contains(&update.to);
        // Edge does not exist in IR.
        if update.kind == UpdateKind::Insert && !has_edge {
            return false;
        }
        // Edge exists in IR.
        if update.kind == UpdateKind::Delete && has_edge {
            return false;
        }
        true
    }

    /// `eraseDelBBNode`: erases the block's node from the trees before it is
    /// unlinked from the function.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L328>
    fn erase_del_bb_node(
        &mut self,
        del_bb: BlockId,
    ) {
        if let Some(dt) = &mut self.dt
            && !self.is_recalculating_dom_tree
            && dt.contains(del_bb)
        {
            dt.erase_node(del_bb);
        }
        if let Some(pdt) = &mut self.pdt
            && !self.is_recalculating_post_dom_tree
            && pdt.contains(del_bb)
        {
            pdt.erase_node(del_bb);
        }
    }

    /// `tryFlushDeletedBB`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L340>
    fn try_flush_deleted_bb(&mut self) {
        if !self.has_pending_updates() {
            self.force_flush_deleted_bb();
        }
    }

    /// `DomTreeUpdater::forceFlushDeletedBB`: the blocks awaiting deletion
    /// leave the trees.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/DomTreeUpdater.cpp#L36>
    fn force_flush_deleted_bb(&mut self) -> bool {
        if self.deleted_bbs.is_empty() {
            return false;
        }
        let blocks: Vec<BlockId> = self.deleted_bbs.iter().collect();
        for bb in blocks {
            self.erase_del_bb_node(bb);
        }
        self.deleted_bbs.clear();
        true
    }

    /// `DomTreeUpdater::deleteBB`: `del_bb` has no predecessor and is about
    /// to be erased from the function. Eager removes it from the trees at
    /// once, Lazy at the next flush event.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/DomTreeUpdater.cpp#L58>
    pub fn delete_bb(
        &mut self,
        del_bb: BlockId,
    ) {
        if self.strategy == UpdateStrategy::Lazy {
            self.deleted_bbs.insert(del_bb);
            return;
        }
        self.erase_del_bb_node(del_bb);
    }

    /// `dropOutOfDateUpdates`: drops the updates every available tree has
    /// applied, and deletes blocks if all available trees are up to date.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L347>
    fn drop_out_of_date_updates(&mut self) {
        if self.strategy == UpdateStrategy::Eager {
            return;
        }
        self.try_flush_deleted_bb();

        // Drop all updates applied by both trees.
        if self.dt.is_none() {
            self.pend_dt_update_index = self.pend_updates.len();
        }
        if self.pdt.is_none() {
            self.pend_pdt_update_index = self.pend_updates.len();
        }
        let drop_index = self.pend_dt_update_index.min(self.pend_pdt_update_index);
        self.pend_updates.drain(..drop_index);
        // Calculate current index.
        self.pend_dt_update_index -= drop_index;
        self.pend_pdt_update_index -= drop_index;
    }

    /// `splitDTCriticalEdges`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L372>
    fn split_dt_critical_edges(
        &mut self,
        graph: &impl DomGraph,
        edges: &[CriticalEdge],
    ) {
        // Bail out early if there is nothing to do.
        let Some(dt) = &mut self.dt else { return };
        if edges.is_empty() {
            return;
        }
        // Remember all the basic blocks that are inserted during edge
        // splitting.
        // Invariant: NewBBs == all the basic blocks contained in the NewBB
        // field of all the elements of Edges.
        let new_bbs: IdSet<BlockId> = edges.iter().map(|edge| edge.new).collect();
        // For each element in Edges, remember whether or not element is the
        // new immediate dominator of its successor. The mapping is done by
        // index.
        let mut is_new_idom = vec![true; edges.len()];

        // Collect all the dominance properties info, before invalidating the
        // underlying DT.
        for (idx, edge) in edges.iter().enumerate() {
            // Update dominator information.
            let succ = edge.to;
            for mut pred_bb in graph.predecessors(succ) {
                if pred_bb == edge.new {
                    continue;
                }
                // If we are in this situation:
                // FromBB1        FromBB2
                //    + +
                //   + +            + +
                //  + +          +   +
                // ...  Split1  Split2 ...
                //           + +
                //            + +
                //             +
                //            Succ
                // Instead of checking the dominance property with Split2, we
                // check it with FromBB2 since Split2 is still unknown of the
                // underlying DT structure.
                if new_bbs.contains(&pred_bb) {
                    let preds = graph.predecessors(pred_bb);
                    assert!(
                        preds.len() == 1,
                        "A basic block resulting from a critical edge split has more than one predecessor!"
                    );
                    pred_bb = preds[0];
                }
                if !dt.dominates(succ, pred_bb) {
                    is_new_idom[idx] = false;
                    break;
                }
            }
        }

        // Now, update DT with the collected dominance properties info.
        for (idx, edge) in edges.iter().enumerate() {
            // We know FromBB dominates NewBB.
            dt.add_new_block(graph, edge.new, edge.from);
            // If all the other predecessors of "Succ" are dominated by "Succ"
            // itself then the new block is the new immediate dominator of
            // "Succ". Otherwise, the new block doesn't dominate anything.
            if is_new_idom[idx] {
                dt.change_immediate_dominator(edge.to, edge.new);
            }
        }
    }

    /// `splitPDTCriticalEdges`: the post-dominator tree is different, the new
    /// block in a critical edge may become the new root.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdaterImpl.h#L433>
    fn split_pdt_critical_edges(
        &mut self,
        graph: &impl DomGraph,
        edges: &[CriticalEdge],
    ) {
        // Bail out early if there is nothing to do.
        let Some(pdt) = &mut self.pdt else { return };
        if edges.is_empty() {
            return;
        }
        let mut updates: Vec<Update> = Vec::new();
        for edge in edges {
            updates.push(Update::insert(edge.from, edge.new));
            updates.push(Update::insert(edge.new, edge.to));
            if !graph.successors(edge.from).contains(&edge.to) {
                updates.push(Update::delete(edge.from, edge.to));
            }
        }
        pdt.apply_updates(graph, &updates);
    }
}

impl Drop for DomTreeUpdater {
    /// `~DomTreeUpdater`: pending updates must have been flushed, by
    /// `into_trees` (the graph is not at hand here).
    fn drop(&mut self) {
        debug_assert!(
            std::thread::panicking() || !self.has_pending_updates(),
            "Pending updates were not flushed by derived class."
        );
    }
}
