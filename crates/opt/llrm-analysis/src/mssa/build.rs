//! `MemorySSA::buildMemorySSA` and what it calls, from
//! [`MemorySSA.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp).

use std::collections::BTreeSet;

use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::module::{BlockId, Function, InstId};

use super::{AccessId, BlockAccesses, Kind, LIVE_ON_ENTRY, MemoryAccess, Mssa};
use crate::cfg::{self, Dominance};

/// What `AA.getModRefInfo(I, std::nullopt)` and `isOrdered(I)` say of an
/// instruction: whether it may read memory, may write it, and whether it is
/// ordered (a volatile access is a def).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Effect {
    pub reads: bool,
    pub writes: bool,
    pub ordered: bool,
}

/// Where `insertIntoListsForBlock` puts an access (`InsertionPlace`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Place {
    Beginning,
    End,
}

impl Mssa {
    /// [`MemorySSA::MemorySSA`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1236)
    /// and
    /// [`buildMemorySSA`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1522)
    /// over the whole function.
    pub fn build(
        function: &Function,
        dominance: &Dominance,
        modref: &dyn Fn(InstId) -> Effect,
    ) -> Self {
        let entry = *function.layout().first().expect("a function with a block");
        // We create an access to represent "live on entry", for things like
        // arguments or users of globals, where the memory they use is
        // defined before the beginning of the function.
        let mut mssa = Self {
            accesses: Vec::new(),
            per_block: IdMap::new(),
            value_to_access: IdMap::new(),
            dominance: dominance.clone(),
            next_id: 0,
            block_numbering: Default::default(),
            block_numbering_valid: Default::default(),
        };
        let id = mssa.next_id;
        mssa.next_id += 1;
        mssa.accesses.push(MemoryAccess {
            kind: Kind::Def,
            block: entry,
            inst: None,
            defining: None,
            incoming: Vec::new(),
            optimized: None,
            users: Vec::new(),
            phi_users: Vec::new(),
            user_slot: 0,
            id,
        });
        debug_assert_eq!(mssa.accesses.len() - 1, LIVE_ON_ENTRY as usize);

        // We maintain lists of memory accesses per-block, trading memory for
        // time. Go through each block, figure out where defs occur, and
        // chain together all the accesses.
        let mut defining_blocks: BTreeSet<i64> = BTreeSet::new();
        for &block in function.layout() {
            let mut insert_into_def = false;
            for &inst in function.block(block).instructions() {
                let Some(access) = mssa.create_new_access(block, inst, modref) else { continue };
                let lists = mssa.per_block.get_or_insert_with(block, BlockAccesses::default);
                lists.accesses.push(access);
                if mssa.accesses[access as usize].kind == Kind::Def {
                    insert_into_def = true;
                    mssa.per_block.get_or_insert_with(block, BlockAccesses::default).defs.push(access);
                }
            }
            if insert_into_def {
                defining_blocks.insert(cfg::id(block));
            }
        }
        mssa.place_phi_nodes(function, &defining_blocks);

        // Now do regular SSA renaming on the MemoryDef/MemoryUse. `visited`
        // will be filled in with all blocks.
        let mut visited: IdSet<BlockId> = IdSet::new();
        mssa.rename_pass(function, entry, LIVE_ON_ENTRY, &mut visited);

        // Mark the uses in unreachable blocks as live on entry, so that they go
        // somewhere.
        for &block in function.layout() {
            if !visited.contains(&block) {
                mssa.mark_unreachable_as_live_on_entry(function, block);
            }
        }
        mssa
    }

    /// [`MemorySSA::createNewAccess`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1761),
    /// without a template and without `isUseTriviallyOptimizableToLiveOnEntry`
    /// (stage 2).
    fn create_new_access(
        &mut self,
        block: BlockId,
        inst: InstId,
        modref: &dyn Fn(InstId) -> Effect,
    ) -> Option<AccessId> {
        let effect = modref(inst);
        // It's possible for an instruction to not modify memory at all. During
        // construction, we ignore them. The ordered check ensures
        // volatiles end up as defs.
        let def = effect.writes || effect.ordered;
        let used = effect.reads;
        if !def && !used {
            return None;
        }
        let (kind, id) = if def {
            let id = self.next_id;
            self.next_id += 1;
            (Kind::Def, id)
        } else {
            (Kind::Use, 0)
        };
        let access = self.accesses.len() as AccessId;
        self.accesses.push(MemoryAccess {
            kind,
            block,
            inst: Some(inst),
            defining: None,
            incoming: Vec::new(),
            optimized: None,
            users: Vec::new(),
            phi_users: Vec::new(),
            user_slot: 0,
            id,
        });
        self.value_to_access.insert(inst, access);
        Some(access)
    }

    /// [`MemorySSA::createMemoryPhi`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1719).
    pub(crate) fn create_memory_phi(
        &mut self,
        block: BlockId,
    ) -> AccessId {
        assert!(self.get_memory_phi(block).is_none(), "MemoryPhi already exists for this BB");
        let id = self.next_id;
        self.next_id += 1;
        let phi = self.accesses.len() as AccessId;
        self.accesses.push(MemoryAccess {
            kind: Kind::Phi,
            block,
            inst: None,
            defining: None,
            incoming: Vec::new(),
            optimized: None,
            users: Vec::new(),
            phi_users: Vec::new(),
            user_slot: 0,
            id,
        });
        // Phi's always are placed at the front of the block.
        self.insert_into_lists_for_block(phi, block, Place::Beginning);
        phi
    }

    /// [`MemorySSA::insertIntoListsForBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1621).
    pub(crate) fn insert_into_lists_for_block(
        &mut self,
        new: AccessId,
        block: BlockId,
        point: Place,
    ) {
        let kind = self.accesses[new as usize].kind;
        let is_phi = |mssa: &Self, access: AccessId| mssa.accesses[access as usize].kind == Kind::Phi;
        let mut lists = std::mem::take(self.per_block.get_or_insert_with(block, BlockAccesses::default));
        match point {
            Place::Beginning => {
                // If it's a phi node, it goes first, otherwise, it goes after
                // any phi nodes.
                if kind == Kind::Phi {
                    lists.accesses.insert(0, new);
                    lists.defs.insert(0, new);
                } else {
                    let at = lists.accesses.iter().position(|one| !is_phi(self, *one)).unwrap_or(lists.accesses.len());
                    lists.accesses.insert(at, new);
                    if kind != Kind::Use {
                        let at = lists.defs.iter().position(|one| !is_phi(self, *one)).unwrap_or(lists.defs.len());
                        lists.defs.insert(at, new);
                    }
                }
            }
            Place::End => {
                lists.accesses.push(new);
                if kind != Kind::Use {
                    lists.defs.push(new);
                }
            }
        }
        *self.per_block.get_or_insert_with(block, BlockAccesses::default) = lists;
        self.block_numbering_valid.borrow_mut().remove(&block);
    }

    /// [`MemorySSA::placePHINodes`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1508):
    /// the iterated dominance frontier of the blocks that define
    /// (`ForwardIDFCalculator`).
    fn place_phi_nodes(
        &mut self,
        function: &Function,
        defining_blocks: &BTreeSet<i64>,
    ) {
        let frontiers = self.dominance.frontiers(function);
        let mut placed: BTreeSet<i64> = BTreeSet::new();
        let mut work: Vec<i64> = defining_blocks.iter().copied().collect();
        let mut queued: BTreeSet<i64> = defining_blocks.clone();
        while let Some(at) = work.pop() {
            for &frontier in &frontiers[&at] {
                if placed.insert(frontier) && queued.insert(frontier) {
                    work.push(frontier);
                }
            }
        }
        // Now place MemoryPhi nodes, in the function's block order.
        for &block in function.layout() {
            if placed.contains(&cfg::id(block)) {
                self.create_memory_phi(block);
            }
        }
    }

    /// [`MemorySSA::renameSuccessorPhis`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1098)
    /// (`RenameAllUses` is false).
    fn rename_successor_phis(
        &mut self,
        function: &Function,
        block: BlockId,
        incoming: AccessId,
    ) {
        // Pass through values to our successors.
        for successor in function.successors(block) {
            // Rename the phi nodes in our successor block.
            let Some(phi) = self.get_memory_phi(successor) else { continue };
            self.add_incoming(phi, incoming, block);
        }
    }

    /// [`MemorySSA::renameBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1125):
    /// the new incoming value.
    fn rename_block(
        &mut self,
        block: BlockId,
        mut incoming: AccessId,
    ) -> AccessId {
        // Skip most processing if the list is empty.
        let Some(list) = self.get_block_accesses(block).map(<[AccessId]>::to_vec) else { return incoming };
        for access in list {
            match self.accesses[access as usize].kind {
                Kind::Use | Kind::Def => {
                    if self.accesses[access as usize].defining.is_none() {
                        self.set_defining_access(access, Some(incoming));
                    }
                    if self.accesses[access as usize].kind == Kind::Def {
                        incoming = access;
                    }
                }
                Kind::Phi => incoming = access,
            }
        }
        incoming
    }

    /// [`MemorySSA::renamePass`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1149):
    /// the standard SSA renaming algorithm, a preorder walk of the dominator
    /// tree (`SkipVisited` and `RenameAllUses` are false).
    fn rename_pass(
        &mut self,
        function: &Function,
        root: BlockId,
        incoming: AccessId,
        visited: &mut IdSet<BlockId>,
    ) {
        let mut children: IdMap<BlockId, Vec<BlockId>> = IdMap::new();
        for &block in function.layout() {
            if let Some(idom) = self.dominance.immediate(cfg::id(block)) {
                children.get_or_insert_with(cfg::block(idom), Vec::new).push(block);
            }
        }
        struct RenamePassData {
            node: BlockId,
            child: usize,
            incoming: AccessId,
        }
        let mut work: Vec<RenamePassData> = Vec::new();
        visited.insert(root);
        let incoming = self.rename_block(root, incoming);
        self.rename_successor_phis(function, root, incoming);
        work.push(RenamePassData { node: root, child: 0, incoming });
        while let Some(top) = work.last_mut() {
            let kids = children.get(&top.node).map_or(&[][..], Vec::as_slice);
            if top.child == kids.len() {
                work.pop();
                continue;
            }
            let block = kids[top.child];
            top.child += 1;
            let from = top.incoming;
            visited.insert(block);
            let incoming = self.rename_block(block, from);
            self.rename_successor_phis(function, block, incoming);
            work.push(RenamePassData { node: block, child: 0, incoming });
        }
    }

    /// [`MemorySSA::markUnreachableAsLiveOnEntry`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1199):
    /// phi nodes in unreachable blocks are deleted, all other unreachable
    /// accesses become uses of live on entry.
    fn mark_unreachable_as_live_on_entry(
        &mut self,
        function: &Function,
        block: BlockId,
    ) {
        assert!(!self.dominance.reachable(cfg::id(block)), "reachable block found while handling unreachable blocks");
        // Make sure phi nodes in our reachable successors end up with a
        // LiveOnEntryDef for our incoming edge, even though our block
        // is forward unreachable.
        for successor in function.successors(block) {
            if !self.dominance.reachable(cfg::id(successor)) {
                continue;
            }
            let Some(phi) = self.get_memory_phi(successor) else { continue };
            self.add_incoming(phi, LIVE_ON_ENTRY, block);
        }
        let Some(list) = self.get_block_accesses(block).map(<[AccessId]>::to_vec) else { return };
        for access in list {
            // If we have a phi, just remove it. We are going to replace all
            // users with live on entry.
            if self.accesses[access as usize].kind == Kind::Phi {
                self.remove_from_lists(access);
            } else {
                self.set_defining_access(access, Some(LIVE_ON_ENTRY));
            }
        }
    }

    /// [`MemorySSA::removeFromLists`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1869)
    /// (the lookup tables are left alone: `removeFromLookups` is stage 3).
    pub(crate) fn remove_from_lists(
        &mut self,
        access: AccessId,
    ) {
        let block = self.accesses[access as usize].block;
        if let Some(lists) = self.per_block.get_mut(&block) {
            lists.accesses.retain(|one| *one != access);
            lists.defs.retain(|one| *one != access);
        }
        self.block_numbering_valid.borrow_mut().remove(&block);
    }
}
