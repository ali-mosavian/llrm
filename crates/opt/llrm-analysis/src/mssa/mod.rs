//! MemorySSA, translated from LLVM's `MemorySSA.cpp` and `MemorySSA.h` at
//! [`d1106deb71cc`](https://github.com/llvm/llvm-project/tree/d1106deb71cc81016406a1917d0508d2df296266).
//! Plan and per-function map: docs/architecture/memoryssa-port.md. This module
//! is shadow-only: nothing in the pipeline reads it.
//!
//! The API is LLVM's (`MemoryDef`, `MemoryUse`, `MemoryPhi`, live on entry,
//! defining access, per-block access and def lists in order); dense ids and
//! `Vec` are the representation under it.

pub mod alias;
pub mod build;
pub mod shadow;
pub mod verify;
pub mod walker;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

use std::cell::RefCell;

use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::module::{BlockId, InstId};

use crate::cfg::Dominance;

/// A position in `Mssa::accesses`. LLVM's `MemoryAccess *`.
pub type AccessId = u32;

/// LLVM's `LiveOnEntryDef`: a `MemoryDef` with no instruction.
pub const LIVE_ON_ENTRY: AccessId = 0;

/// [`MemoryAccess`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L140)'s
/// three kinds
/// ([`MemoryUse`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L310),
/// [`MemoryDef`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L371),
/// [`MemoryPhi`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L479)).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Use,
    Def,
    Phi,
}

#[derive(Clone, Debug)]
pub struct MemoryAccess {
    pub kind: Kind,
    /// `getBlock`.
    pub block: BlockId,
    /// `MemoryUseOrDef::getMemoryInst`: none for a phi and for live on entry.
    pub inst: Option<InstId>,
    /// `getDefiningAccess` (operand 0): none for live on entry and for a phi.
    pub defining: Option<AccessId>,
    /// `MemoryPhi`: `(getIncomingBlock(i), getIncomingValue(i))`.
    pub incoming: Vec<(BlockId, AccessId)>,
    /// A def's optimized access (operand 1), none for a use (whose optimized access is its
    /// defining access).
    pub optimized: Option<AccessId>,
    /// `OptimizedID`: the id of the access this one was optimized to.
    pub optimized_id: Option<u32>,
    /// `MemoryAccess::users`: the accesses that use this one as an operand.
    pub users: Vec<AccessId>,
    /// `MemoryDef::getID` and `MemoryPhi::getID`.
    pub id: u32,
}

/// `AccessList` and `DefsList` of one block
/// ([`MemorySSA.h#L753`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L753)).
#[derive(Clone, Debug, Default)]
pub struct BlockAccesses {
    pub accesses: Vec<AccessId>,
    pub defs: Vec<AccessId>,
}

/// [`MemorySSA`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L702).
pub struct Mssa {
    pub(crate) accesses: Vec<MemoryAccess>,
    /// `PerBlockAccesses` and `PerBlockDefs`.
    pub(crate) per_block: IdMap<BlockId, BlockAccesses>,
    /// `ValueToMemoryAccess`.
    pub(crate) value_to_access: IdMap<InstId, AccessId>,
    pub(crate) dominance: Dominance,
    pub(crate) next_id: u32,
    /// `BlockNumbering` and `BlockNumberingValid`.
    pub(crate) block_numbering: RefCell<IdMap<AccessId, u32>>,
    pub(crate) block_numbering_valid: RefCell<IdSet<BlockId>>,
}

impl Mssa {
    /// `getLiveOnEntryDef`.
    pub fn live_on_entry_def(&self) -> AccessId {
        LIVE_ON_ENTRY
    }

    /// `isLiveOnEntryDef`.
    pub fn is_live_on_entry_def(
        &self,
        access: AccessId,
    ) -> bool {
        access == LIVE_ON_ENTRY
    }

    /// The number of accesses, live on entry and phis included.
    pub fn len(&self) -> usize {
        self.accesses.len()
    }

    pub fn is_empty(&self) -> bool {
        self.accesses.is_empty()
    }

    pub fn access(
        &self,
        access: AccessId,
    ) -> &MemoryAccess {
        &self.accesses[access as usize]
    }

    /// `getMemoryAccess(const Instruction *)`.
    pub fn get_memory_access(
        &self,
        inst: InstId,
    ) -> Option<AccessId> {
        self.value_to_access.get(&inst).copied()
    }

    /// `getMemoryAccess(const BasicBlock *)`: the block's phi.
    pub fn get_memory_phi(
        &self,
        block: BlockId,
    ) -> Option<AccessId> {
        let first = *self.per_block.get(&block)?.accesses.first()?;
        (self.accesses[first as usize].kind == Kind::Phi).then_some(first)
    }

    /// `getBlockAccesses`.
    pub fn get_block_accesses(
        &self,
        block: BlockId,
    ) -> Option<&[AccessId]> {
        self.per_block.get(&block).map(|lists| lists.accesses.as_slice())
    }

    /// `getBlockDefs`.
    pub fn get_block_defs(
        &self,
        block: BlockId,
    ) -> Option<&[AccessId]> {
        self.per_block.get(&block).map(|lists| lists.defs.as_slice()).filter(|defs| !defs.is_empty())
    }

    /// `getDefiningAccess`.
    pub fn get_defining_access(
        &self,
        access: AccessId,
    ) -> Option<AccessId> {
        self.accesses[access as usize].defining
    }

    /// `MemoryUseOrDef::setDefiningAccess`: the use list of the old and the new
    /// operand follow.
    pub(crate) fn set_defining_access(
        &mut self,
        access: AccessId,
        new: Option<AccessId>,
    ) {
        if let Some(old) = self.accesses[access as usize].defining {
            let users = &mut self.accesses[old as usize].users;
            if let Some(at) = users.iter().position(|one| *one == access) {
                users.swap_remove(at);
            }
        }
        self.accesses[access as usize].defining = new;
        if let Some(new) = new {
            self.accesses[new as usize].users.push(access);
        }
    }

    /// `MemoryUseOrDef::setOptimized`: for a use, `setOptimized` also sets operand 0 (the defining
    /// access); for a def, operand 1.
    pub(crate) fn set_optimized(
        &mut self,
        access: AccessId,
        to: AccessId,
    ) {
        let id = self.accesses[to as usize].id;
        if self.accesses[access as usize].kind == Kind::Use {
            self.set_defining_access(access, Some(to));
        } else {
            if let Some(old) = self.accesses[access as usize].optimized {
                let users = &mut self.accesses[old as usize].users;
                if let Some(at) = users.iter().position(|one| *one == access) {
                    users.swap_remove(at);
                }
            }
            self.accesses[access as usize].optimized = Some(to);
            self.accesses[to as usize].users.push(access);
        }
        self.accesses[access as usize].optimized_id = Some(id);
    }

    /// `MemoryUseOrDef::getOptimized`.
    pub fn get_optimized(
        &self,
        access: AccessId,
    ) -> Option<AccessId> {
        let one = self.access(access);
        if one.kind == Kind::Use { one.defining } else { one.optimized }
    }

    /// `MemoryUseOrDef::isOptimized`.
    pub fn is_optimized(
        &self,
        access: AccessId,
    ) -> bool {
        let one = self.access(access);
        self.get_optimized(access).is_some_and(|target| one.optimized_id == Some(self.access(target).id))
    }

    /// `MemoryUseOrDef::resetOptimized`.
    pub(crate) fn reset_optimized(
        &mut self,
        access: AccessId,
    ) {
        self.accesses[access as usize].optimized_id = None;
        if self.accesses[access as usize].kind != Kind::Use {
            if let Some(old) = self.accesses[access as usize].optimized.take() {
                let users = &mut self.accesses[old as usize].users;
                if let Some(at) = users.iter().position(|one| *one == access) {
                    users.swap_remove(at);
                }
            }
        }
    }

    /// `MemoryPhi::addIncoming`.
    pub(crate) fn add_incoming(
        &mut self,
        phi: AccessId,
        value: AccessId,
        block: BlockId,
    ) {
        self.accesses[phi as usize].incoming.push((block, value));
        self.accesses[value as usize].users.push(phi);
    }

    /// [`MemorySSA::renumberBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2132).
    fn renumber_block(
        &self,
        block: BlockId,
    ) {
        let list = self.get_block_accesses(block).expect("asking to renumber an empty block");
        let mut numbering = self.block_numbering.borrow_mut();
        for (at, access) in list.iter().enumerate() {
            numbering.insert(*access, at as u32 + 1);
        }
        self.block_numbering_valid.borrow_mut().insert(block);
    }

    /// [`MemorySSA::locallyDominates`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2145).
    pub fn locally_dominates(
        &self,
        dominator: AccessId,
        dominatee: AccessId,
    ) -> bool {
        let block = self.access(dominator).block;
        assert!(
            block == self.access(dominatee).block,
            "asking for local domination when accesses are in different blocks"
        );
        // A node dominates itself.
        if dominatee == dominator {
            return true;
        }
        // When the dominatee is defined on function entry, no other access
        // dominates it.
        if self.is_live_on_entry_def(dominatee) {
            return false;
        }
        // When the dominator is defined on function entry, it dominates the
        // other.
        if self.is_live_on_entry_def(dominator) {
            return true;
        }
        if !self.block_numbering_valid.borrow().contains(&block) {
            self.renumber_block(block);
        }
        let numbering = self.block_numbering.borrow();
        numbering.get(&dominator).expect("a numbered access") < numbering.get(&dominatee).expect("a numbered access")
    }

    /// [`MemorySSA::dominates`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2176)`(MemoryAccess,
    /// MemoryAccess)`.
    pub fn dominates(
        &self,
        dominator: AccessId,
        dominatee: AccessId,
    ) -> bool {
        if dominator == dominatee {
            return true;
        }
        if self.is_live_on_entry_def(dominatee) {
            return false;
        }
        let (a, b) = (self.access(dominator).block, self.access(dominatee).block);
        if a != b {
            return self.dominance.dominates(crate::cfg::id(a), crate::cfg::id(b));
        }
        self.locally_dominates(dominator, dominatee)
    }

    /// [`MemorySSA::dominates`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2189)`(MemoryAccess,
    /// Use)`: the use is `user`'s operand, a phi's incoming edge `incoming`;
    /// for a phi the used value (`cast<MemoryAccess>(Dominatee)` of the `Use`)
    /// is the incoming value.
    pub fn dominates_use(
        &self,
        dominator: AccessId,
        user: AccessId,
        incoming: Option<usize>,
    ) -> bool {
        let user_access = self.access(user);
        if let (Kind::Phi, Some(at)) = (user_access.kind, incoming) {
            let use_block = user_access.incoming[at].0;
            // The def must dominate the incoming block of the phi.
            if use_block != self.access(dominator).block {
                return self
                    .dominance
                    .dominates(crate::cfg::id(self.access(dominator).block), crate::cfg::id(use_block));
            }
            // If the use block and the def block are the same, compare locally.
            return self.locally_dominates(dominator, user_access.incoming[at].1);
        }
        self.dominates(dominator, user)
    }
}
