//! What a client that asks MemorySSA of instructions (EarlyCSE) calls:
//! `getMemoryAccess`, `getWalker()->getClobberingMemoryAccess` (one access, the
//! `MemoryPhi` where the clobbers diverge), `getDefiningAccess`, `dominates`.

use std::cell::RefCell;

use llrm_mir::module::InstId;

use super::alias::AliasQuery;
use super::updater::MemorySsaUpdater;
use super::{AccessId, Mssa};
use crate::memory::Unit;
use crate::memoryssa::Accesses;

/// The translated graph and the alias query its walker asks.
pub struct Translated<'a> {
    mssa: RefCell<Mssa>,
    updater: RefCell<MemorySsaUpdater>,
    aa: AliasQuery<'a>,
}

impl<'a> Translated<'a> {
    pub fn new(
        unit: &'a Unit<'a>,
        accesses: &'a Accesses,
        mssa: Mssa,
    ) -> Self {
        Self {
            mssa: RefCell::new(mssa),
            updater: RefCell::new(MemorySsaUpdater::new()),
            aa: AliasQuery::new(unit, accesses),
        }
    }

    /// `MemorySSAUpdater::removeMemoryAccess(inst)`, before `inst` goes from
    /// `function`: what used its access reads what it read.
    pub fn remove_memory_access(
        &self,
        function: &llrm_mir::module::Function,
        inst: InstId,
    ) {
        let mut mssa = self.mssa.borrow_mut();
        if let Some(access) = mssa.get_memory_access(inst) {
            self.updater.borrow_mut().remove_memory_access(&mut mssa, function, access, true);
        }
    }

    /// The graph as the edits made through this view left it.
    pub fn into_mssa(self) -> Mssa {
        self.mssa.into_inner()
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
