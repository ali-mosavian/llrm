//! What `MemorySSA.cpp` asks of `AliasAnalysis`, answered per query from the
//! syntactic references of `Accesses::plain`: nothing is built for the whole
//! function. `AAQueryInfo`'s cache
//! ([`AliasAnalysis.h#L239`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/AliasAnalysis.h#L239))
//! is the `(def, location)` map here.
//!
//! DEVIATE: LLVM's `MemoryLocation` is `{Ptr, Size, AAInfo}`; here a location
//! is a `MemRef` (the pointer decomposed, with its type lineage) and a flag for
//! a size made unknown (`LocationSize::beforeOrAfterPointer`).

use std::cell::RefCell;
use std::rc::Rc;

use llrm_mir::dense::IdMap;
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
    /// `LocationSize::beforeOrAfterPointer`: any access before or after the
    /// pointer clobbers.
    pub unbounded: bool,
    /// What an overlap test is asked of: the reference, its width made large
    /// when unbounded.
    effective: MemRef,
    /// This location with its size made unknown, made once.
    wider: std::cell::OnceCell<Rc<Location>>,
}

/// `AAResults` / `BatchAAResults` as MemorySSA uses them.
pub struct AliasQuery<'a> {
    unit: &'a Unit<'a>,
    accesses: &'a Accesses,
    next_id: std::cell::Cell<u32>,
    by_instruction: RefCell<IdMap<InstId, Option<Rc<Location>>>>,
    /// `AAQueryInfo::AliasCache`: the answer for each (def, location) asked.
    cache: RefCell<llrm_support::hash::HashMap<u64, bool>>,
    /// Questions asked of `mod_set_loc`, and those it had to work out.
    asked: std::cell::Cell<(usize, usize)>,
    /// Entries of the cache looked at to answer the questions: one a question,
    /// for the test that holds the cost linear.
    cache_steps: std::cell::Cell<u64>,
}

/// A width large enough to stand for 'any byte after the pointer'.
const UNBOUNDED_WIDTH: u32 = u32::MAX >> 2;

impl<'a> AliasQuery<'a> {
    pub fn new(
        unit: &'a Unit<'a>,
        accesses: &'a Accesses,
    ) -> Self {
        Self {
            unit,
            accesses,
            next_id: Default::default(),
            by_instruction: Default::default(),
            cache: Default::default(),
            asked: Default::default(),
            cache_steps: Default::default(),
        }
    }

    /// Cache entries looked at to answer the questions so far.
    pub fn cache_steps(&self) -> u64 {
        self.cache_steps.get()
    }

    /// How many `mod_set_loc` questions were asked and how many were worked out
    /// (the rest were in the cache).
    pub fn questions(&self) -> (usize, usize) {
        self.asked.get()
    }

    pub fn unit(&self) -> &Unit<'a> {
        self.unit
    }

    /// A new location for `reference`; a location is told apart from another by
    /// its number, so no `MemRef` is hashed.
    pub fn location_for(
        &self,
        reference: &MemRef,
        unbounded: bool,
    ) -> Rc<Location> {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let effective =
            if unbounded { MemRef { width: UNBOUNDED_WIDTH, ..reference.clone() } } else { reference.clone() };
        Rc::new(Location { id, reference: reference.clone(), unbounded, effective, wider: Default::default() })
    }

    /// `getWithNewSize(LocationSize::beforeOrAfterPointer())`.
    pub fn widened(
        &self,
        loc: &Rc<Location>,
    ) -> Rc<Location> {
        if loc.unbounded {
            return Rc::clone(loc);
        }
        Rc::clone(loc.wider.get_or_init(|| self.location_for(&loc.reference, true)))
    }

    /// `MemoryLocation::get(Inst)`: none where the instruction has no single
    /// location (a call, a fence), and where the references hold none.
    pub fn location_of(
        &self,
        inst: InstId,
    ) -> Option<Rc<Location>> {
        if let Some(found) = self.by_instruction.borrow().get(&inst) {
            return found.clone();
        }
        let made = match self.unit.function.instruction(inst).opcode {
            Opcode::Load { .. } | Opcode::Store { .. } => {
                self.accesses.references.get(&inst).map(|reference| self.location_for(reference, false))
            }
            _ => None,
        };
        self.by_instruction.borrow_mut().insert(inst, made.clone());
        made
    }

    /// `isModSet(AA.getModRefInfo(DefInst, UseLoc))`: whether `def` may write a
    /// byte of `loc`.
    pub fn mod_set_loc(
        &self,
        def: InstId,
        loc: &Location,
    ) -> bool {
        let (asked, worked) = self.asked.get();
        self.asked.set((asked + 1, worked));
        let key = (u64::from(def.0) << 32) | u64::from(loc.id);
        self.cache_steps.set(self.cache_steps.get() + 1);
        if let Some(&known) = self.cache.borrow().get(&key) {
            return known;
        }
        self.asked.set((asked + 1, worked + 1));
        let writes = self.accesses.writes(def);
        let found =
            memoryssa::changes(&loc.effective, false, writes, |written| self.may_overlap(&loc.effective, written));
        self.cache.borrow_mut().insert(key, found);
        found
    }

    /// `isModSet(AA.getModRefInfo(DefInst, CB))`: whether `def` may write a
    /// byte `call` reads or writes.
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
            Some(list) => list.iter().any(|one| writes.iter().any(|w| self.may_overlap(one, w))),
        };
        touched(self.accesses.reads(call)) || touched(self.accesses.writes(call))
    }

    /// The identified object (a global or an alloca) `reference` points into:
    /// `getUnderlyingObject` and `isIdentifiedObject`.
    fn identified_object(
        &self,
        reference: &MemRef,
    ) -> Option<llrm_mir::module::Operand> {
        use llrm_mir::context::ConstantKind;
        use llrm_mir::module::{Operand, ValueDef};
        let base = if reference.object {
            reference.root?
        } else {
            let unit = self.unit;
            llrm_mir::valuetracking::underlying(unit.context, unit.layout, unit.function, reference.pointer?).0
        };
        let identified = match base {
            Operand::Constant(id) => matches!(self.unit.context.get(id).kind, ConstantKind::Global(_)),
            Operand::Value(value) => match self.unit.function.value(value).def {
                ValueDef::Instruction(def) => {
                    matches!(
                        self.unit.function.instruction(def).opcode,
                        Opcode::Alloca { .. }
                    )
                }
                ValueDef::Argument(_) => false,
            },
            Operand::Block(_) => false,
        };
        identified.then_some(base)
    }

    /// `AliasAnalysis::alias` of two references, as far as the stand-in goes:
    /// two identified objects that differ do not overlap (BasicAA), else the
    /// overlap test of `may_clobber`.
    fn may_overlap(
        &self,
        one: &MemRef,
        other: &MemRef,
    ) -> bool {
        if one.space == other.space
            && let (Some(left), Some(right)) = (self.identified_object(one), self.identified_object(other))
            && left != right
        {
            return false;
        }
        memoryssa::may_clobber(self.unit, None, one, other)
    }

    /// `AA.getMemoryEffects(CB)`: whether the call may read and may write
    /// memory.
    pub fn call_effects(
        &self,
        call: InstId,
    ) -> (bool, bool) {
        (
            self.accesses.reads(call).is_none_or(|reads| !reads.is_empty()),
            self.accesses.writes(call).is_none_or(|writes| !writes.is_empty()),
        )
    }

    /// `!isNoModRef(AA.getModRefInfo(I1, I2))`
    /// ([`AliasAnalysis.cpp#L390`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/AliasAnalysis.cpp#L390)):
    /// whether `first` may read or write a byte `second` reads or writes.
    pub fn mod_ref_instructions(
        &self,
        first: InstId,
        second: InstId,
    ) -> bool {
        let accesses = self.accesses;
        let touched = |inst: InstId| -> Option<Vec<&MemRef>> {
            let (reads, writes) = (accesses.reads(inst)?, accesses.writes(inst)?);
            Some(reads.iter().chain(writes).collect())
        };
        let (Some(first_touched), Some(second_touched)) = (touched(first), touched(second)) else {
            // One may touch anything: it conflicts unless the other touches
            // nothing.
            let nothing = |inst: InstId| {
                accesses.reads(inst).is_some_and(<[_]>::is_empty) && accesses.writes(inst).is_some_and(<[_]>::is_empty)
            };
            return !(nothing(first) || nothing(second));
        };
        first_touched.iter().any(|one| second_touched.iter().any(|other| self.may_overlap(one, other)))
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
        let volatile =
            |inst: InstId| matches!(
                self.unit.function.instruction(inst).opcode,
                Opcode::Load { volatile: true, .. }
            );
        // Volatile operations may never be reordered with other volatile
        // operations.
        !(volatile(used) && volatile(may_clobber))
    }
}
