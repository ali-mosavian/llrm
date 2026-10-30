//! `MemorySSAUpdater` (`MemorySSAUpdater.cpp`, `d1106deb71cc`): keeps the graph
//! right as instructions are inserted, moved and erased, in place of making it
//! again. The edits of the instruction level, the ones LICM and DSE make;
//! edits of the CFG are not here yet.
//!
//! The updater carries three sets between its calls
//! (`VisitedBlocks`, `InsertedPHIs`, `NonOptPhis`); the graph and the function
//! (for its edges) are given to each call.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::module::{BlockId, Function, InstId};

use super::build::{Effect, Place};
use super::edit::Where;
use super::{AccessId, Kind, LIVE_ON_ENTRY, Mssa};
use crate::cfg;

/// `MemorySSAUpdater`.
#[derive(Default)]
pub struct MemorySsaUpdater {
    visited_blocks: IdSet<BlockId>,
    inserted_phis: Vec<AccessId>,
    non_opt_phis: BTreeSet<AccessId>,
    /// What each access removed in the call stood for (the `ValueHandle`
    /// replacement `removeMemoryAccess` makes): the tracking handle of
    /// `recursePhi` follows it.
    replaced_by: BTreeMap<AccessId, AccessId>,
}

/// Where `MemorySSAUpdater::moveTo*` puts an access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveTo {
    Before(AccessId),
    After(AccessId),
    Beginning,
    End,
    BeforeTerminator,
}

/// `getUniquePredecessor`: the one block that precedes `block`, however many
/// edges.
fn unique_predecessor(
    function: &Function,
    block: BlockId,
) -> Option<BlockId> {
    let predecessors = function.predecessors(block);
    let first = *predecessors.first()?;
    predecessors.iter().all(|&one| one == first).then_some(first)
}

impl MemorySsaUpdater {
    pub fn new() -> Self {
        Self::default()
    }

    /// The access `access` stands for now (it is itself where it still stands).
    fn current(
        &self,
        mssa: &Mssa,
        mut access: AccessId,
    ) -> AccessId {
        while mssa.access(access).dead {
            match self.replaced_by.get(&access) {
                Some(&next) => access = next,
                None => break,
            }
        }
        access
    }

    /// [`getPreviousDefIterative`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L41):
    /// the definition that reaches the end of `block`, phis made where paths
    /// meet, found with an explicit work list.
    fn get_previous_def_iterative(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        block: BlockId,
        cached: &mut IdMap<BlockId, AccessId>,
    ) -> AccessId {
        #[derive(PartialEq)]
        enum Resume {
            EnterBlock,
            ResumeSinglePred,
            RunPredLoop,
        }
        struct Frame {
            block: BlockId,
            resume: Resume,
            phi_ops: Vec<AccessId>,
            predecessors: Vec<BlockId>,
            pred_at: usize,
            pending_incoming: bool,
            unique_incoming_access: bool,
            single_access: Option<AccessId>,
        }
        impl Frame {
            fn new(
                function: &Function,
                block: BlockId,
            ) -> Self {
                Self {
                    block,
                    resume: Resume::EnterBlock,
                    phi_ops: Vec::new(),
                    predecessors: function.predecessors(block).to_vec(),
                    pred_at: 0,
                    pending_incoming: false,
                    unique_incoming_access: true,
                    single_access: None,
                }
            }

            /// Fold an incoming predecessor access into this frame's phi
            /// operands, tracking whether all incoming accesses are identical
            /// (so the phi may be elided).
            fn incorporate(
                &mut self,
                incoming: AccessId,
            ) {
                match self.single_access {
                    None => self.single_access = Some(incoming),
                    Some(single) if single != incoming => self.unique_incoming_access = false,
                    Some(_) => {}
                }
                self.phi_ops.push(incoming);
            }
        }
        // If `pred` has a local definition, cache and return its last def.
        let local_def_from_end = |mssa: &Mssa, cached: &mut IdMap<BlockId, AccessId>, pred: BlockId| {
            let defs = mssa.get_block_defs(pred)?;
            let result = *defs.last().expect("a def");
            cached.insert(pred, result);
            Some(result)
        };
        let mut stack: Vec<Frame> = vec![Frame::new(function, block)];
        // Carries a completed child frame's result back to its parent frame.
        let mut returned: Option<AccessId> = None;
        while let Some(top) = stack.len().checked_sub(1) {
            let current = stack[top].block;
            match stack[top].resume {
                Resume::EnterBlock => {
                    // First, do a cache lookup.
                    if let Some(&found) = cached.get(&current) {
                        returned = Some(self.current(mssa, found));
                        stack.pop();
                        continue;
                    }
                    // If this method is called from an unreachable block,
                    // return LoE.
                    if !mssa.dominance.reachable(function, cfg::id(current)) {
                        returned = Some(LIVE_ON_ENTRY);
                        stack.pop();
                        continue;
                    }
                    if let Some(pred) = unique_predecessor(function, current) {
                        self.visited_blocks.insert(current);
                        // Single predecessor case, there can be only one
                        // definition. If pred has a
                        // local def take it, otherwise descend into pred.
                        if let Some(result) = local_def_from_end(mssa, cached, pred) {
                            cached.insert(current, result);
                            returned = Some(result);
                            stack.pop();
                            continue;
                        }
                        stack[top].resume = Resume::ResumeSinglePred;
                        stack.push(Frame::new(function, pred));
                        continue;
                    }
                    if self.visited_blocks.contains(&current) {
                        // We hit our node again, meaning we had a cycle, we
                        // must insert a phi node to
                        // break it so we have an operand.
                        let result = mssa.create_memory_phi(current);
                        cached.insert(current, result);
                        returned = Some(result);
                        stack.pop();
                        continue;
                    }
                    // Mark us visited so we can detect a cycle, then walk the
                    // predecessors.
                    self.visited_blocks.insert(current);
                    stack[top].resume = Resume::RunPredLoop;
                }
                Resume::ResumeSinglePred => {
                    // The single predecessor's result is in `returned`.
                    cached.insert(current, self.current(mssa, returned.expect("a result")));
                    stack.pop();
                }
                Resume::RunPredLoop => {
                    if stack[top].pending_incoming {
                        let incoming = self.current(mssa, returned.expect("a result"));
                        stack[top].incorporate(incoming);
                        stack[top].pending_incoming = false;
                        stack[top].pred_at += 1;
                    }
                    let mut suspended = false;
                    while stack[top].pred_at < stack[top].predecessors.len() {
                        let pred = stack[top].predecessors[stack[top].pred_at];
                        if mssa.dominance.reachable(function, cfg::id(pred)) {
                            // Local def resolves now, otherwise descend into
                            // pred.
                            if let Some(incoming) = local_def_from_end(mssa, cached, pred) {
                                stack[top].incorporate(incoming);
                            } else {
                                stack[top].pending_incoming = true;
                                stack.push(Frame::new(function, pred));
                                suspended = true;
                                break;
                            }
                        } else {
                            stack[top].phi_ops.push(LIVE_ON_ENTRY);
                        }
                        stack[top].pred_at += 1;
                    }
                    if suspended {
                        continue;
                    }
                    // Now try to simplify the ops to avoid placing a phi. This
                    // may return none if we never created a
                    // phi yet, that's okay.
                    let mut phi = mssa.get_memory_phi(current);
                    // Handles on phis removed since follow them.
                    let phi_ops: Vec<AccessId> = std::mem::take(&mut stack[top].phi_ops)
                        .into_iter()
                        .map(|one| self.current(mssa, one))
                        .collect();
                    let unique = stack[top].unique_incoming_access;
                    let single = stack[top].single_access.map(|one| self.current(mssa, one));
                    let predecessors = std::mem::take(&mut stack[top].predecessors);
                    // See if we can avoid the phi by simplifying it.
                    let mut result = self.try_remove_trivial_phi_ops(mssa, function, phi, &phi_ops);
                    // If we couldn't simplify, we may have to create a phi.
                    let unchanged = result == phi;
                    let result = if unchanged && unique && single.is_some() {
                        // A concrete phi only exists if we created an empty one
                        // to break a cycle.
                        let single = single.expect("a single access");
                        if let Some(phi) = phi {
                            assert!(mssa.access(phi).incoming.is_empty(), "expected empty phi");
                            self.replaced_by.insert(phi, single);
                            mssa.replace_all_uses_with(phi, single);
                            self.remove_memory_access(mssa, function, phi, true);
                        }
                        single
                    } else if unchanged {
                        let phi_id = match phi {
                            Some(phi) => phi,
                            None => mssa.create_memory_phi(current),
                        };
                        phi = Some(phi_id);
                        // See if the existing phi operands match what we need.
                        // Unlike normal SSA, we only
                        // allow one phi node per block, so we can't
                        // just create a new one.
                        if !mssa.access(phi_id).incoming.is_empty() {
                            let same = mssa
                                .access(phi_id)
                                .incoming
                                .iter()
                                .map(|&(_, value)| value)
                                .eq(phi_ops.iter().copied());
                            if !same {
                                for at in (0..mssa.access(phi_id).incoming.len()).rev() {
                                    mssa.unordered_delete_incoming_value(phi_id, at);
                                }
                                for (&pred, &value) in predecessors.iter().zip(&phi_ops) {
                                    mssa.add_incoming(phi_id, value, pred);
                                }
                            }
                        } else {
                            for (&pred, &value) in predecessors.iter().zip(&phi_ops) {
                                mssa.add_incoming(phi_id, value, pred);
                            }
                            self.inserted_phis.push(phi_id);
                        }
                        phi_id
                    } else {
                        result.take().expect("a result")
                    };
                    // Set ourselves up for the next variable by resetting
                    // visited state.
                    self.visited_blocks.remove(&current);
                    cached.insert(current, result);
                    returned = Some(result);
                    stack.pop();
                }
            }
        }
        self.current(mssa, returned.expect("a definition"))
    }

    /// [`getPreviousDef`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L252):
    /// the definition before `access`, in its block or, with phis made where
    /// they are needed, from before it.
    pub fn get_previous_def(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        access: AccessId,
    ) -> AccessId {
        if let Some(local) = self.get_previous_def_in_block(mssa, access) {
            return local;
        }
        let mut cached: IdMap<BlockId, AccessId> = IdMap::new();
        self.get_previous_def_iterative(mssa, function, mssa.access(access).block, &mut cached)
    }

    /// [`getPreviousDefInBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L262):
    /// the definition before `access` in its block, none if the block has none
    /// there.
    fn get_previous_def_in_block(
        &self,
        mssa: &Mssa,
        access: AccessId,
    ) -> Option<AccessId> {
        let block = mssa.access(access).block;
        // It's possible there are no defs, or we got handed the first def to
        // start.
        let defs = mssa.get_block_defs(block)?;
        if mssa.access(access).kind != Kind::Use {
            // If this is a def, we can just use the def list.
            let at = defs.iter().position(|&one| one == access)?;
            return at.checked_sub(1).map(|before| defs[before]);
        }
        // Otherwise, have to walk the all-access list backwards.
        let list = mssa.get_block_accesses(block)?;
        let at = list.iter().position(|&one| one == access)?;
        list[..at].iter().rev().copied().find(|&one| mssa.access(one).kind != Kind::Use)
    }

    /// [`getPreviousDefFromEnd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L287).
    fn get_previous_def_from_end(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        block: BlockId,
        cached: &mut IdMap<BlockId, AccessId>,
    ) -> AccessId {
        if let Some(defs) = mssa.get_block_defs(block) {
            let last = *defs.last().expect("a def");
            cached.insert(block, last);
            return last;
        }
        self.get_previous_def_iterative(mssa, function, block, cached)
    }

    /// [`recursePhi`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L300):
    /// the phis that use `phi`, tried for triviality; `phi` as it stands after.
    fn recurse_phi(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        phi: Option<AccessId>,
    ) -> Option<AccessId> {
        let phi = phi?;
        let mut users = mssa.access(phi).phi_users.clone();
        users.dedup();
        users.sort_unstable();
        users.dedup();
        for user in users {
            let user = self.current(mssa, user);
            if !mssa.access(user).dead && mssa.access(user).kind == Kind::Phi {
                self.try_remove_trivial_phi(mssa, function, user);
            }
        }
        Some(self.current(mssa, phi))
    }

    /// [`tryRemoveTrivialPhi`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L317):
    /// a phi whose operands are all itself or one access is that access.
    pub fn try_remove_trivial_phi(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        phi: AccessId,
    ) -> AccessId {
        let operands: Vec<AccessId> = mssa.access(phi).incoming.iter().map(|&(_, value)| value).collect();
        self.try_remove_trivial_phi_ops(mssa, function, Some(phi), &operands).expect("a phi or its replacement")
    }

    /// `tryRemoveTrivialPhi(Phi, Operands)`: the phi itself where it cannot go.
    fn try_remove_trivial_phi_ops(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        phi: Option<AccessId>,
        operands: &[AccessId],
    ) -> Option<AccessId> {
        // Bail out on non-opt phis.
        if phi.is_some_and(|phi| self.non_opt_phis.contains(&phi)) {
            return phi;
        }
        // Detect equal or self arguments.
        let mut same: Option<AccessId> = None;
        for &operand in operands {
            // If the same or self, good so far.
            if Some(operand) == phi || Some(operand) == same {
                continue;
            }
            // Not the same, return the phi since it's not eliminatable by us.
            if same.is_some() {
                return phi;
            }
            same = Some(operand);
        }
        // Never found a non-self reference, the phi is undef.
        let Some(same) = same else { return Some(LIVE_ON_ENTRY) };
        if let Some(phi) = phi {
            // The handles on the phi follow it to what replaces it.
            self.replaced_by.insert(phi, same);
            mssa.replace_all_uses_with(phi, same);
            self.remove_memory_access(mssa, function, phi, true);
        }
        // We should only end up recursing in case we replaced something, in
        // which case, we may have made other phis trivial.
        self.recurse_phi(mssa, function, Some(same))
    }

    /// [`insertUse`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L353):
    /// the use `mu`, put in its block, gets its defining access.
    pub fn insert_use(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        mu: AccessId,
        rename_uses: bool,
    ) {
        self.visited_blocks.clear();
        self.inserted_phis.clear();
        let before = self.get_previous_def(mssa, function, mu);
        mssa.set_defining_access(mu, Some(before));
        // In cases without unreachable blocks, because uses do not create new
        // may-defs, there are only two cases: there was a def already below us,
        // and therefore, we should not have created a phi node because it was
        // already needed for the def; or there is no def below us, and
        // therefore, there is no extra renaming work to do. In cases
        // with unreachable blocks, where the unnecessary phis were
        // optimized out, adding the use may re-insert those phis.
        // Hence, when inserting uses outside of the MSSA creation
        // process, and new phis were added, rename all uses if we
        // are asked.
        if rename_uses && !self.inserted_phis.is_empty() {
            let mut visited: IdSet<BlockId> = IdSet::new();
            let start = mssa.access(mu).block;
            if let Some(defs) = mssa.get_block_defs(start) {
                let mut first = defs[0];
                // Convert to incoming value if it's a memorydef. A phi *is*
                // already an incoming value.
                if mssa.access(first).kind == Kind::Def {
                    first = mssa.access(first).defining.expect("a defining access");
                }
                mssa.rename_pass(function, start, Some(first), &mut visited, true, true);
            }
            // We just inserted a phi into this block, so the incoming value
            // will become the phi anyway, so it does not matter
            // what we pass.
            for phi in self.inserted_phis.clone() {
                let phi = self.current(mssa, phi);
                if !mssa.access(phi).dead && mssa.access(phi).kind == Kind::Phi {
                    let block = mssa.access(phi).block;
                    mssa.rename_pass(function, block, None, &mut visited, true, true);
                }
            }
        }
    }

    /// `setMemoryPhiValueForBlock`: every incoming edge `{block, phi's block}`
    /// of `phi` is `new_def`.
    fn set_memory_phi_value_for_block(
        mssa: &mut Mssa,
        phi: AccessId,
        block: BlockId,
        new_def: AccessId,
    ) {
        // Replace any operand with us an incoming block with the new defining
        // access.
        let index = mssa.get_basic_block_index(phi, block).expect("the block in the phi");
        let mut at = index;
        while at < mssa.access(phi).incoming.len() && mssa.access(phi).incoming[at].0 == block {
            mssa.set_incoming_value(phi, at, new_def);
            at += 1;
        }
        // Edges of one block that are not next to each other.
        for at in 0..mssa.access(phi).incoming.len() {
            if mssa.access(phi).incoming[at].0 == block {
                mssa.set_incoming_value(phi, at, new_def);
            }
        }
    }

    /// [`insertDef`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L422):
    /// the def `md`, put in its block, becomes the definition the accesses
    /// after it read, and the phis it needs are made.
    pub fn insert_def(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        md: AccessId,
        rename_uses: bool,
    ) {
        let md_block = mssa.access(md).block;
        // Don't bother updating dead code.
        if !mssa.dominance.reachable(function, cfg::id(md_block)) {
            mssa.set_defining_access(md, Some(LIVE_ON_ENTRY));
            return;
        }
        self.visited_blocks.clear();
        self.inserted_phis.clear();
        // See if we had a local def, and if not, go hunting.
        let def_before = self.get_previous_def(mssa, function, md);
        let def_before_same_block = mssa.access(def_before).block == md_block
            && !(mssa.access(def_before).kind == Kind::Phi && self.inserted_phis.contains(&def_before));
        // There is a def before us, which means we can replace any store/phi
        // uses of that thing with us, since we are in the way of
        // whatever was there before. We now define that def's
        // memorydefs and memoryphis.
        if def_before_same_block {
            // Leave the memory uses alone, and ourselves, to avoid self
            // references. Defs are automatically unoptimized when
            // the user is set to `md` below, because `isOptimized`
            // fails to find the same ID.
            mssa.replace_uses_with_if(def_before, md, &|mssa, user| mssa.access(user).kind == Kind::Use || user == md);
        }
        // And that def is now our defining access.
        mssa.set_defining_access(md, Some(def_before));
        let mut fixup_list: Vec<AccessId> = self.inserted_phis.clone();
        let mut existing_phis: BTreeSet<AccessId> = BTreeSet::new();
        // Remember the index where we may insert new phis.
        let mut new_phi_index = self.inserted_phis.len();
        if !def_before_same_block {
            // If there was a local def before us, we must have the same effect
            // it did. If there was no local def before us, we
            // performed a global update, and have to search all
            // successors and make sure we update the first def in
            // each of them (following all paths until we hit the first
            // def along each path). This may also insert phi nodes.
            //
            // Compute the IDF in all cases, as renaming needs to be done even
            // when `md` is not the last access, because it can
            // introduce a new access past which a previous access
            // was optimized; that access needs to be reoptimized.
            let mut defining_blocks: BTreeSet<i64> = BTreeSet::new();
            defining_blocks.insert(cfg::id(md_block));
            for &phi in &self.inserted_phis {
                if !mssa.access(phi).dead {
                    defining_blocks.insert(cfg::id(mssa.access(phi).block));
                }
            }
            let idf_blocks = mssa.iterated_dominance_frontier(function, &defining_blocks);
            let mut new_inserted_phis: Vec<AccessId> = Vec::new();
            for block in idf_blocks {
                let phi = match mssa.get_memory_phi(block) {
                    Some(existing) => {
                        existing_phis.insert(existing);
                        existing
                    }
                    None => {
                        let made = mssa.create_memory_phi(block);
                        new_inserted_phis.push(made);
                        made
                    }
                };
                // Add the phis created into the IDF blocks to `non_opt_phis`,
                // so they are not optimized out as trivial by
                // the call to `get_previous_def_from_end`
                // below. Once they are complete, all these phis
                // are added to the fixup list, and removed from
                // `non_opt_phis` inside `fixup_defs`.
                self.non_opt_phis.insert(phi);
            }
            for &phi in &new_inserted_phis {
                let block = mssa.access(phi).block;
                for pred in function.predecessors(block) {
                    let mut cached: IdMap<BlockId, AccessId> = IdMap::new();
                    let value = self.get_previous_def_from_end(mssa, function, pred, &mut cached);
                    mssa.add_incoming(phi, value, pred);
                }
            }
            // Re-take the index where we're adding the new phis, because the
            // above call to `get_previous_def_from_end` may have
            // inserted into `inserted_phis`.
            new_phi_index = self.inserted_phis.len();
            for &phi in &new_inserted_phis {
                self.inserted_phis.push(phi);
                fixup_list.push(phi);
            }
            fixup_list.push(md);
        }
        // Update defining access of following defs.
        let new_phi_index_end = self.inserted_phis.len();
        self.fixup_defs(mssa, function, &fixup_list);
        assert_eq!(new_phi_index_end, self.inserted_phis.len(), "should not insert new phis during fixup_defs");
        // Optimize potentially non-minimal phis added in this method.
        for phi in self.inserted_phis[new_phi_index..new_phi_index_end].to_vec() {
            let phi = self.current(mssa, phi);
            if !mssa.access(phi).dead && mssa.access(phi).kind == Kind::Phi {
                self.try_remove_trivial_phi(mssa, function, phi);
            }
        }
        // The answers the walker kept for defs may now pass a clobber.
        mssa.reset_optimized_defs();
        // Now that all fixups are done, rename all uses if we are asked. The
        // defs are guaranteed to be in reachable code due to the check
        // at the method entry.
        if rename_uses {
            let mut visited: IdSet<BlockId> = IdSet::new();
            // We are guaranteed there is a def in the block, because we just
            // got it handed to us in this function.
            let mut first = mssa.get_block_defs(md_block).expect("a def in the block")[0];
            // Convert to incoming value if it's a memorydef. A phi *is* already
            // an incoming value.
            if mssa.access(first).kind == Kind::Def {
                first = mssa.access(first).defining.expect("a defining access");
            }
            mssa.rename_pass(function, md_block, Some(first), &mut visited, true, true);
            // We just inserted a phi into this block, so the incoming value
            // will become the phi anyway, so it does not matter
            // what we pass.
            for phi in self.inserted_phis.clone() {
                let phi = self.current(mssa, phi);
                if !mssa.access(phi).dead && mssa.access(phi).kind == Kind::Phi {
                    let block = mssa.access(phi).block;
                    mssa.rename_pass(function, block, None, &mut visited, true, true);
                }
            }
            // Existing phi blocks may need renaming too, if an access was
            // previously optimized and the inserted defs "cover"
            // the optimized value.
            for phi in existing_phis {
                let phi = self.current(mssa, phi);
                if !mssa.access(phi).dead && mssa.access(phi).kind == Kind::Phi {
                    let block = mssa.access(phi).block;
                    mssa.rename_pass(function, block, None, &mut visited, true, true);
                }
            }
        }
    }

    /// [`fixupDefs`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L571):
    /// the first def after each of `vars` along every path reads it.
    fn fixup_defs(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        vars: &[AccessId],
    ) {
        let mut seen: IdSet<BlockId> = IdSet::new();
        let mut worklist: Vec<BlockId> = Vec::new();
        for &var in vars {
            let new_def = self.current(mssa, var);
            if mssa.access(new_def).dead {
                continue;
            }
            let block = mssa.access(new_def).block;
            // First, see if there is a local def after the operand.
            let defs = mssa.get_block_defs(block).map(<[AccessId]>::to_vec).unwrap_or_default();
            let at = defs.iter().position(|&one| one == new_def).expect("the new def in its block");
            // The temporary phi is being fixed, unmark it for not to optimize.
            if mssa.access(new_def).kind == Kind::Phi {
                self.non_opt_phis.remove(&new_def);
            }
            // If there is a local def after us, we only have to rename that.
            if let Some(&next) = defs.get(at + 1) {
                mssa.set_defining_access(next, Some(new_def));
                continue;
            }
            // Otherwise, we need to search down through the CFG. For each of
            // our successors, handle it directly if there is a phi,
            // or place on the fixup worklist.
            for successor in function.successors(block) {
                match mssa.get_memory_phi(successor) {
                    Some(phi) => Self::set_memory_phi_value_for_block(mssa, phi, block, new_def),
                    None => worklist.push(successor),
                }
            }
            while let Some(fixup_block) = worklist.pop() {
                // Get the first def in the block that isn't a phi node.
                if let Some(defs) = mssa.get_block_defs(fixup_block) {
                    let first = defs[0];
                    // The loop above and below should have taken care of phi
                    // nodes.
                    assert!(mssa.access(first).kind != Kind::Phi, "should have already handled phi nodes");
                    // We are now this def's defining access, make sure we
                    // actually dominate it.
                    assert!(mssa.dominates(new_def, first), "should have dominated the new access");
                    mssa.set_defining_access(first, Some(new_def));
                    continue;
                }
                // We didn't find a def, so we must continue.
                for successor in function.successors(fixup_block) {
                    // If there is a phi node, handle it. Otherwise, put the
                    // block on the worklist.
                    match mssa.get_memory_phi(successor) {
                        Some(phi) => Self::set_memory_phi_value_for_block(mssa, phi, fixup_block, new_def),
                        None => {
                            // If we cycle, we should have ended up at a phi
                            // node that we
                            // already processed.
                            if !seen.insert(successor) {
                                continue;
                            }
                            worklist.push(successor);
                        }
                    }
                }
            }
        }
    }

    /// `tryRemoveTrivialPhis`.
    pub fn try_remove_trivial_phis(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        phis: &[AccessId],
    ) {
        for &phi in phis {
            let phi = self.current(mssa, phi);
            if !mssa.access(phi).dead && mssa.access(phi).kind == Kind::Phi {
                self.try_remove_trivial_phi(mssa, function, phi);
            }
        }
    }

    /// [`moveTo`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L1262):
    /// `what` is taken out of the graph where it stood and put at `to` in
    /// `block`.
    fn move_to_internal(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        what: AccessId,
        block: BlockId,
        to: MoveTo,
    ) {
        // Mark memory phi users of `what` not to be optimized.
        for phi in mssa.access(what).phi_users.clone() {
            self.non_opt_phis.insert(phi);
        }
        // Replace all our users with our defining access.
        let defining = mssa.access(what).defining.expect("a defining access");
        mssa.replace_all_uses_with(what, defining);
        // Let the graph take care of moving it around in the lists.
        match to {
            MoveTo::Before(access) => mssa.move_to_before(what, block, Where::Before(access)),
            MoveTo::After(access) => {
                let list = mssa.get_block_accesses(block).expect("a placed access").to_vec();
                let at = list.iter().position(|&one| one == access).expect("a placed access");
                let place = list.get(at + 1).map_or(Where::End, |&next| Where::Before(next));
                // `what` itself is not in the list yet when asked by value; it
                // may be `next`.
                let place = if place == Where::Before(what) {
                    list.get(at + 2).map_or(Where::End, |&next| Where::Before(next))
                } else {
                    place
                };
                mssa.move_to_before(what, block, place)
            }
            MoveTo::Beginning => mssa.move_to_place(what, block, Place::Beginning),
            MoveTo::End => mssa.move_to_place(what, block, Place::End),
            MoveTo::BeforeTerminator => unreachable!("resolved by the caller"),
        }
        // Now reinsert it into the IR and do whatever fixups needed.
        if mssa.access(what).kind == Kind::Def {
            self.insert_def(mssa, function, what, true);
        } else {
            self.insert_use(mssa, function, what, true);
        }
        // Clear dangling pointers. We added all memory phi users, but not all
        // of them are removed by fixup_defs.
        self.non_opt_phis.clear();
    }

    /// `moveBefore`.
    pub fn move_before(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        what: AccessId,
        before: AccessId,
    ) {
        let block = mssa.access(before).block;
        self.move_to_internal(mssa, function, what, block, MoveTo::Before(before));
    }

    /// `moveAfter`.
    pub fn move_after(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        what: AccessId,
        after: AccessId,
    ) {
        let block = mssa.access(after).block;
        self.move_to_internal(mssa, function, what, block, MoveTo::After(after));
    }

    /// [`moveToPlace`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L1296):
    /// `to` is the beginning or the end of `block`, or before its terminator
    /// (whose access, if it has one, is `terminator`).
    pub fn move_to_place(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        what: AccessId,
        block: BlockId,
        to: MoveTo,
        terminator: Option<AccessId>,
    ) {
        let to = match to {
            MoveTo::BeforeTerminator => match terminator {
                Some(access) => MoveTo::Before(access),
                None => MoveTo::End,
            },
            other => other,
        };
        self.move_to_internal(mssa, function, what, block, to);
    }

    /// [`removeMemoryAccess`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L1401):
    /// `ma` goes; what used it reads what it read.
    pub fn remove_memory_access(
        &mut self,
        mssa: &mut Mssa,
        function: &Function,
        ma: AccessId,
        optimize_phis: bool,
    ) {
        assert!(!mssa.is_live_on_entry_def(ma), "trying to remove the live on entry def");
        // We can only delete phi nodes if they have no uses, or we can replace
        // all uses with a single definition.
        let new_def_target: Option<AccessId> = if mssa.access(ma).kind == Kind::Phi {
            // It is sufficient to know that all edges of the phi node have the
            // same argument: by the definition of dominance
            // frontiers (which we used to place this phi), that
            // argument must dominate this phi, and thus, must
            // dominate the phi's uses.
            let only = only_single_value(mssa, ma);
            assert!(only.is_some() || !mssa.has_uses(ma), "we can't delete this memory phi");
            only
        } else {
            mssa.access(ma).defining
        };
        let mut phis_to_check: Vec<AccessId> = Vec::new();
        // Re-point the uses at our defining access.
        if mssa.access(ma).kind != Kind::Use && mssa.has_uses(ma) {
            let target = new_def_target.expect("a target for the uses");
            assert_ne!(target, ma, "going into an infinite loop");
            self.replaced_by.insert(ma, target);
            // Reset optimized on the users of this store, and reset the uses.
            for user in mssa.access(ma).users.clone() {
                if mssa.access(user).kind != Kind::Phi {
                    mssa.reset_optimized(user);
                }
                mssa.set_defining_access(user, Some(target));
            }
            for phi in mssa.access(ma).phi_users.clone() {
                if optimize_phis && !phis_to_check.contains(&phi) {
                    phis_to_check.push(phi);
                }
                for at in 0..mssa.access(phi).incoming.len() {
                    if mssa.access(phi).incoming[at].1 == ma {
                        mssa.set_incoming_value(phi, at, target);
                    }
                }
            }
            for def in mssa.access(ma).optimized_users.clone() {
                mssa.reset_optimized(def);
            }
        }
        // The access is deleted below: do things in the right order.
        mssa.remove_from_lookups(ma);
        mssa.remove_from_lists_deleting(ma, true);
        // Optionally optimize phi uses. This will recursively remove trivial
        // phis.
        while let Some(phi) = phis_to_check.pop() {
            let phi = self.current(mssa, phi);
            if !mssa.access(phi).dead && mssa.access(phi).kind == Kind::Phi {
                self.try_remove_trivial_phi(mssa, function, phi);
            }
        }
    }

    /// [`createMemoryAccessInBB`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSAUpdater.cpp#L1525):
    /// an access for `inst`, put at the beginning or end of `block`; it still
    /// has to be inserted (`insert_use`, `insert_def`).
    pub fn create_memory_access_in_bb(
        &self,
        mssa: &mut Mssa,
        inst: InstId,
        definition: Option<AccessId>,
        block: BlockId,
        point: Place,
        modref: &dyn Fn(InstId) -> Effect,
    ) -> Option<AccessId> {
        let access = mssa.create_defined_access(block, inst, definition, modref)?;
        mssa.insert_into_lists_for_block(access, block, point);
        Some(access)
    }

    /// `createMemoryAccessBefore`.
    pub fn create_memory_access_before(
        &self,
        mssa: &mut Mssa,
        inst: InstId,
        definition: Option<AccessId>,
        insert_point: AccessId,
        modref: &dyn Fn(InstId) -> Effect,
    ) -> Option<AccessId> {
        let block = mssa.access(insert_point).block;
        let access = mssa.create_defined_access(block, inst, definition, modref)?;
        mssa.insert_into_lists_before(access, block, Where::Before(insert_point));
        Some(access)
    }

    /// `createMemoryAccessAfter`.
    pub fn create_memory_access_after(
        &self,
        mssa: &mut Mssa,
        inst: InstId,
        definition: Option<AccessId>,
        insert_point: AccessId,
        modref: &dyn Fn(InstId) -> Effect,
    ) -> Option<AccessId> {
        let block = mssa.access(insert_point).block;
        let access = mssa.create_defined_access(block, inst, definition, modref)?;
        let list = mssa.get_block_accesses(block).expect("a placed access").to_vec();
        let at = list.iter().position(|&one| one == insert_point).expect("a placed access");
        let place = list.get(at + 1).map_or(Where::End, |&next| Where::Before(next));
        mssa.insert_into_lists_before(access, block, place);
        Some(access)
    }
}

/// `onlySingleValue`: the one access all of a phi's incoming values are.
fn only_single_value(
    mssa: &Mssa,
    phi: AccessId,
) -> Option<AccessId> {
    let mut found: Option<AccessId> = None;
    for &(_, value) in &mssa.access(phi).incoming {
        match found {
            None => found = Some(value),
            Some(seen) if seen != value => return None,
            Some(_) => {}
        }
    }
    found
}
