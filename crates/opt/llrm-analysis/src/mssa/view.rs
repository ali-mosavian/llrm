//! What a client that asks MemorySSA of instructions (EarlyCSE) calls:
//! `getMemoryAccess`, `getWalker()->getClobberingMemoryAccess` (one access, the
//! `MemoryPhi` where the clobbers diverge), `getDefiningAccess`, `dominates`.

use std::cell::RefCell;

use llrm_mir::module::InstId;

use super::alias::AliasQuery;
use super::{AccessId, Mssa};
use crate::memory::Unit;
use crate::memoryssa::Accesses;

/// The translated graph and the alias query its walker asks.
pub struct Translated<'a> {
    mssa: RefCell<Mssa>,
    aa: AliasQuery<'a>,
}

impl<'a> Translated<'a> {
    pub fn new(
        unit: &'a Unit<'a>,
        accesses: &'a Accesses,
        mssa: Mssa,
    ) -> Self {
        Self { mssa: RefCell::new(mssa), aa: AliasQuery::new(unit, accesses) }
    }

    /// `MemorySSA::getMemoryAccess`.
    pub fn memory_access(
        &self,
        inst: InstId,
    ) -> Option<AccessId> {
        self.mssa.borrow().get_memory_access(inst)
    }

    /// `getClobberingMemoryAccess(inst)`: the one access that clobbers, a phi
    /// where the nearest clobbers differ; the defining access where `inst` has
    /// no location to ask by.
    pub fn clobbering_access(
        &self,
        inst: InstId,
    ) -> Option<AccessId> {
        let mut mssa = self.mssa.borrow_mut();
        let access = mssa.get_memory_access(inst)?;
        Some(mssa.get_clobbering_memory_access(&self.aa, access))
    }

    /// `MemoryUseOrDef::getDefiningAccess`.
    pub fn defining_access(
        &self,
        inst: InstId,
    ) -> Option<AccessId> {
        let mssa = self.mssa.borrow();
        mssa.access(mssa.get_memory_access(inst)?).defining
    }

    /// `MemorySSA::dominates`.
    pub fn dominates(
        &self,
        ahead: AccessId,
        access: AccessId,
    ) -> bool {
        self.mssa.borrow().dominates(ahead, access)
    }

    /// Whether `access` is a `MemoryPhi`.
    pub fn is_phi(
        &self,
        access: AccessId,
    ) -> bool {
        self.mssa.borrow().access(access).kind == super::Kind::Phi
    }

    /// The instruction of `access` where it is a def of one (not the
    /// live-on-entry state, not a phi).
    pub fn def_instruction(
        &self,
        access: AccessId,
    ) -> Option<InstId> {
        let mssa = self.mssa.borrow();
        let one = mssa.access(access);
        (one.kind == super::Kind::Def).then_some(one.inst).flatten()
    }
}
