//! What `MemorySSA.cpp` asks of `AliasAnalysis`, answered per query from the syntactic references
//! of `Accesses::plain`: nothing is built for the whole function. `AAQueryInfo`'s cache
//! ([`AliasAnalysis.h#L239`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/AliasAnalysis.h#L239))
//! is the `(def, location)` map here.
//!
//! DEVIATE: LLVM's `MemoryLocation` is `{Ptr, Size, AAInfo}`; here a location is a `MemRef`
//! (the pointer decomposed, with its type lineage) and a flag for a size made unknown
//! (`LocationSize::beforeOrAfterPointer`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::memory::{MemRef, Unit};
use crate::memoryssa::{self, Accesses};

/// `MemoryLocation`.
#[derive(Debug)]
pub struct Location {
    /// The number of this location among those the query has made.
    pub id: u32,
    pub reference: MemRef,
    /// `LocationSize::beforeOrAfterPointer`: any access before or after the pointer clobbers.
    pub unbounded: bool,
    /// What an overlap test is asked of: the reference, its width made large when unbounded.
    effective: MemRef,
}

/// `AAResults` / `BatchAAResults` as MemorySSA uses them.
pub struct AliasQuery<'a> {
    unit: &'a Unit<'a>,
    accesses: &'a Accesses,
    locations: RefCell<HashMap<(MemRef, bool), Rc<Location>>>,
    by_instruction: RefCell<HashMap<InstId, Option<Rc<Location>>>>,
    /// `AAQueryInfo::AliasCache`.
    cache: RefCell<HashMap<(InstId, u32), bool>>,
}

/// A width large enough to stand for 'any byte after the pointer'.
const UNBOUNDED_WIDTH: u32 = u32::MAX >> 2;

impl<'a> AliasQuery<'a> {
    pub fn new(
        unit: &'a Unit<'a>,
        accesses: &'a Accesses,
    ) -> Self {
        Self { unit, accesses, locations: Default::default(), by_instruction: Default::default(), cache: Default::default() }
    }

    pub fn unit(&self) -> &Unit<'a> {
        self.unit
    }

    /// A location for `reference`, the same one for the same reference and size flag.
    pub fn intern(
        &self,
        reference: &MemRef,
        unbounded: bool,
    ) -> Rc<Location> {
        let mut locations = self.locations.borrow_mut();
        let next = locations.len() as u32;
        locations
            .entry((reference.clone(), unbounded))
            .or_insert_with(|| {
                let effective =
                    if unbounded { MemRef { width: UNBOUNDED_WIDTH, ..reference.clone() } } else { reference.clone() };
                Rc::new(Location { id: next, reference: reference.clone(), unbounded, effective })
            })
            .clone()
    }

    /// `MemoryLocation::get(Inst)`: none where the instruction has no single location (a call, a
    /// fence), and where the references hold none.
    pub fn location_of(
        &self,
        inst: InstId,
    ) -> Option<Rc<Location>> {
        if let Some(found) = self.by_instruction.borrow().get(&inst) {
            return found.clone();
        }
        let made = match self.unit.function.instruction(inst).opcode {
            Opcode::Load { .. } | Opcode::Store { .. } => {
                self.accesses.references.get(&inst).map(|reference| self.intern(reference, false))
            }
            _ => None,
        };
        self.by_instruction.borrow_mut().insert(inst, made.clone());
        made
    }

    /// `isModSet(AA.getModRefInfo(DefInst, UseLoc))`: whether `def` may write a byte of `loc`.
    pub fn mod_set_loc(
        &self,
        def: InstId,
        loc: &Location,
    ) -> bool {
        if let Some(known) = self.cache.borrow().get(&(def, loc.id)) {
            return *known;
        }
        let writes = self.accesses.writes(def);
        let found = memoryssa::changes(&loc.effective, false, writes, |written| {
            memoryssa::may_clobber(self.unit, None, &loc.effective, written)
        });
        self.cache.borrow_mut().insert((def, loc.id), found);
        found
    }

    /// `isModSet(AA.getModRefInfo(DefInst, CB))`: whether `def` may write a byte `call` reads or
    /// writes.
    pub fn mod_set_call(
        &self,
        def: InstId,
        call: InstId,
    ) -> bool {
        let Some(writes) = self.accesses.writes(def) else { return true };
        if writes.is_empty() {
            return false;
        }
        let touched = |list: Option<&[MemRef]>| match list {
            None => true,
            Some(list) => list.iter().any(|one| writes.iter().any(|w| memoryssa::may_clobber(self.unit, None, one, w))),
        };
        touched(self.accesses.reads(call)) || touched(self.accesses.writes(call))
    }

    /// [`isUseTriviallyOptimizableToLiveOnEntry`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L364):
    /// an invariant load, or a load of memory nothing writes.
    pub fn is_use_trivially_optimizable_to_live_on_entry(
        &self,
        inst: InstId,
    ) -> bool {
        let unit = self.unit;
        if llrm_mir::memory::invariant_load(unit.context, unit.layout, unit.function, inst) {
            return true;
        }
        matches!(unit.function.instruction(inst).opcode, Opcode::Load { .. })
            && self.location_of(inst).is_some_and(|loc| loc.reference.unwritable())
    }

    /// [`areLoadsReorderable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L249)
    /// (no atomic orderings in MIR loads: only volatility).
    pub fn are_loads_reorderable(
        &self,
        used: InstId,
        may_clobber: InstId,
    ) -> bool {
        let volatile = |inst: InstId| matches!(self.unit.function.instruction(inst).opcode, Opcode::Load { volatile: true, .. });
        // Volatile operations may never be reordered with other volatile operations.
        !(volatile(used) && volatile(may_clobber))
    }
}
