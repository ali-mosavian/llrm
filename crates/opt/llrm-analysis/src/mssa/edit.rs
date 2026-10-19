//! What `MemorySSA` and `MemoryAccess` give `MemorySSAUpdater` to edit the
//! graph with: replacing uses, moving and removing accesses, making one for
//! an instruction.

use llrm_mir::module::{BlockId, InstId};

use super::build::{Effect, Place};
use super::{AccessId, BlockAccesses, Kind, MemoryAccess, Mssa};

/// Where an access goes in its block: before another access of it, or at the
/// end (`AccessList::iterator` or `end()`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Where {
    Before(AccessId),
    End,
}

impl Mssa {
    /// `MemoryPhi::getBasicBlockIndex`: the first edge coming from `block`.
    pub fn get_basic_block_index(
        &self,
        phi: AccessId,
        block: BlockId,
    ) -> Option<usize> {
        self.accesses[phi as usize].incoming.iter().position(|&(from, _)| from == block)
    }

    /// `MemoryPhi::setIncomingValue`: the use lists of the old and the new
    /// value follow.
    pub(crate) fn set_incoming_value(
        &mut self,
        phi: AccessId,
        index: usize,
        value: AccessId,
    ) {
        let old = self.accesses[phi as usize].incoming[index].1;
        if old == value {
            return;
        }
        self.drop_phi_user(old, phi);
        self.accesses[phi as usize].incoming[index].1 = value;
        self.accesses[value as usize].phi_users.push(phi);
    }

    /// One edge of `phi` less in the uses of `value`.
    fn drop_phi_user(
        &mut self,
        value: AccessId,
        phi: AccessId,
    ) {
        let users = &mut self.accesses[value as usize].phi_users;
        if let Some(at) = users.iter().position(|&one| one == phi) {
            users.swap_remove(at);
        }
    }

    /// `MemoryPhi::unorderedDeleteIncomingValue` for the edge `index`.
    pub(crate) fn unordered_delete_incoming_value(
        &mut self,
        phi: AccessId,
        index: usize,
    ) {
        let (_, value) = self.accesses[phi as usize].incoming.swap_remove(index);
        self.drop_phi_user(value, phi);
    }

    /// `MemoryAccess::replaceAllUsesWith`, keeping the users `keep` says
    /// (`replaceUsesWithIf`): `keep` is given the user and whether it takes
    /// `old` as its optimized access (operand 1) or as a phi's incoming
    /// value. What moves is every use of `old` for which `keep` is false. A
    /// user's `OptimizedID` is not changed, so a def that moves is no
    /// longer optimized.
    pub(crate) fn replace_uses_with_if(
        &mut self,
        old: AccessId,
        new: AccessId,
        keep: &dyn Fn(&Mssa, AccessId) -> bool,
    ) {
        if old == new {
            return;
        }
        for user in self.accesses[old as usize].users.clone() {
            if !keep(self, user) {
                self.set_defining_access(user, Some(new));
            }
        }
        for phi in self.accesses[old as usize].phi_users.clone() {
            if keep(self, phi) {
                continue;
            }
            for at in 0..self.accesses[phi as usize].incoming.len() {
                if self.accesses[phi as usize].incoming[at].1 == old {
                    self.set_incoming_value(phi, at, new);
                }
            }
        }
        for def in self.accesses[old as usize].optimized_users.clone() {
            if keep(self, def) {
                continue;
            }
            let id = self.accesses[def as usize].optimized_id;
            self.set_optimized(def, new);
            // `setOptimized` stamped the id of `new`; the def keeps the id it
            // had.
            self.accesses[def as usize].optimized_id = id;
        }
    }

    /// `MemoryAccess::replaceAllUsesWith`.
    pub(crate) fn replace_all_uses_with(
        &mut self,
        old: AccessId,
        new: AccessId,
    ) {
        self.replace_uses_with_if(old, new, &|_, _| false);
    }

    /// The cached optimized access of every def is dropped. DEVIATE
    /// (soundness): a def inserted on a path between a def and the access
    /// its walk found leaves that answer too optimistic, and LLVM's updater
    /// renames uses only (`insertDef`: "Defs are automatically unoptimized
    /// when the user is set ..." covers the defs that read the new one, not
    /// the ones that merely pass it).
    pub(crate) fn reset_optimized_defs(&mut self) {
        for at in 0..self.accesses.len() {
            let one = &self.accesses[at];
            if !one.dead && one.kind == Kind::Def && one.optimized.is_some() {
                self.reset_optimized(at as AccessId);
            }
        }
    }

    /// Whether anything takes `access` as an operand (`use_empty` is false).
    pub fn has_uses(
        &self,
        access: AccessId,
    ) -> bool {
        let one = &self.accesses[access as usize];
        !(one.users.is_empty() && one.phi_users.is_empty() && one.optimized_users.is_empty())
    }

    /// [`MemorySSA::removeFromLookups`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1843).
    pub(crate) fn remove_from_lookups(
        &mut self,
        access: AccessId,
    ) {
        assert!(!self.has_uses(access), "trying to remove memory access that still has uses");
        self.block_numbering.borrow_mut().remove(&access);
        if self.accesses[access as usize].kind != Kind::Phi {
            self.set_defining_access(access, None);
        }
        // Invalidate our walker's cache if necessary.
        if self.accesses[access as usize].kind != Kind::Use {
            self.invalidate_info(access);
        }
        if let Some(inst) = self.accesses[access as usize].inst
            && self.value_to_access.get(&inst) == Some(&access)
        {
            self.value_to_access.remove(&inst);
        }
    }

    /// [`MemorySSA::removeFromLists`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1869)
    /// with `ShouldDelete`: the access is also deleted, which for a phi lets go
    /// of its incoming values.
    pub(crate) fn remove_from_lists_deleting(
        &mut self,
        access: AccessId,
        delete: bool,
    ) {
        self.remove_from_lists(access);
        if !delete {
            return;
        }
        if self.accesses[access as usize].kind == Kind::Phi {
            for (_, value) in std::mem::take(&mut self.accesses[access as usize].incoming) {
                self.drop_phi_user(value, access);
            }
        }
        self.accesses[access as usize].dead = true;
    }

    /// [`MemorySSA::prepareForMoveTo`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1749):
    /// out of the lists, still in the lookups, to be put in a block's.
    fn prepare_for_move_to(
        &mut self,
        what: AccessId,
        block: BlockId,
    ) {
        self.remove_from_lists_deleting(what, false);
        // Moving implicitly invalidates the optimized state of a use; it does
        // not of a def, so it is done here.
        if self.accesses[what as usize].kind == Kind::Def {
            self.reset_optimized(what);
        }
        self.accesses[what as usize].block = block;
    }

    /// [`MemorySSA::insertIntoListsBefore`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1656).
    pub(crate) fn insert_into_lists_before(
        &mut self,
        what: AccessId,
        block: BlockId,
        at: Where,
    ) {
        let kind = self.accesses[what as usize].kind;
        let mut lists = std::mem::take(self.per_block.get_or_insert_with(block, BlockAccesses::default));
        let index = match at {
            Where::End => lists.accesses.len(),
            Where::Before(access) => lists.accesses.iter().position(|&one| one == access).expect("a placed access"),
        };
        lists.accesses.insert(index, what);
        if kind != Kind::Use {
            // Before the next def, or at the end where none follows.
            let next_def = lists.accesses[index + 1..]
                .iter()
                .find(|&&one| self.accesses[one as usize].kind != Kind::Use)
                .and_then(|next| lists.defs.iter().position(|one| one == next));
            lists.defs.insert(next_def.unwrap_or(lists.defs.len()), what);
        }
        *self.per_block.get_or_insert_with(block, BlockAccesses::default) = lists;
        self.block_numbering_valid.borrow_mut().remove(&block);
    }

    /// [`MemorySSA::moveTo`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1772)
    /// before an access: `what` belongs to the lists and the block, but has not
    /// the right defining access, and if it is a def the accesses below it are
    /// not yet updated.
    pub(crate) fn move_to_before(
        &mut self,
        what: AccessId,
        block: BlockId,
        at: Where,
    ) {
        self.prepare_for_move_to(what, block);
        self.insert_into_lists_before(what, block, at);
    }

    /// `MemorySSA::moveTo` to the beginning or the end of the block.
    pub(crate) fn move_to_place(
        &mut self,
        what: AccessId,
        block: BlockId,
        point: Place,
    ) {
        self.prepare_for_move_to(what, block);
        self.insert_into_lists_for_block(what, block, point);
    }

    /// [`MemorySSA::createDefinedAccess`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1761)
    /// (no template, no `isUseTriviallyOptimizableToLiveOnEntry`: stage 2b):
    /// the access of `inst` in `block`, in no list yet, its defining access
    /// `definition`. None where the instruction touches no memory.
    pub(crate) fn create_defined_access(
        &mut self,
        block: BlockId,
        inst: InstId,
        definition: Option<AccessId>,
        modref: &dyn Fn(InstId) -> Effect,
    ) -> Option<AccessId> {
        let access = self.create_new_access(block, inst, modref)?;
        // `createDefinedAccess` always sets the definition (even none).
        self.set_defining_access(access, definition);
        Some(access)
    }

    /// The access in `block` of the first instruction at or after `inst` that
    /// has one: where an access made for an instruction placed before it goes.
    pub fn place_before_next_access(
        &self,
        function: &llrm_mir::module::Function,
        block: BlockId,
        after: Option<InstId>,
    ) -> Where {
        let rest: Box<dyn Iterator<Item = InstId> + '_> =
            match after.filter(|&inst| function.parent(inst) == Some(block)) {
                Some(inst) => Box::new(function.iter_after(inst)),
                None => Box::new(function.iter_block(block)),
            };
        for inst in rest {
            if let Some(access) = self.get_memory_access(inst)
                && self.accesses[access as usize].block == block
                && !self.accesses[access as usize].dead
            {
                return Where::Before(access);
            }
        }
        Where::End
    }
}

impl MemoryAccess {
    /// Whether the access is a memory phi.
    pub fn is_phi(&self) -> bool {
        self.kind == Kind::Phi
    }
}
