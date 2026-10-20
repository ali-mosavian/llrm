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

/// `AliasResult`, without the offset it carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AliasResult {
    No,
    May,
    Partial,
    Must,
}

/// `ModRefInfo`: whether an instruction may read, and may write.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ModRef {
    pub reads: bool,
    pub writes: bool,
}

impl ModRef {
    pub const NONE: ModRef = ModRef { reads: false, writes: false };
    pub const REF: ModRef = ModRef { reads: true, writes: false };
    pub const MOD: ModRef = ModRef { reads: false, writes: true };

    pub fn union(
        self,
        other: ModRef,
    ) -> ModRef {
        ModRef { reads: self.reads || other.reads, writes: self.writes || other.writes }
    }

    pub fn is_mod(self) -> bool {
        self.writes
    }

    pub fn is_ref(self) -> bool {
        self.reads
    }

    pub fn is_mod_or_ref(self) -> bool {
        self.reads || self.writes
    }

    pub fn is_mod_and_ref(self) -> bool {
        self.reads && self.writes
    }
}

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
    /// `BasicAA`'s answers for the pointer pairs asked: one `AAQueryInfo` for
    /// the batch.
    basic: RefCell<crate::aa::AAQueryInfo>,
    /// `getUnderlyingObject` of each pointer asked: the walk down a GEP chain
    /// was a third of a store-heavy body's cost, asked again at each step.
    bases: RefCell<llrm_support::hash::HashMap<llrm_mir::module::Operand, llrm_mir::module::Operand>>,
    /// How many of those walks were made, for the test that holds it to one a
    /// pointer.
    bases_walked: std::cell::Cell<u64>,
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
            basic: RefCell::new(crate::aa::AAQueryInfo::with_capture(Box::<
                crate::captureanalysis::EarliestEscapeAnalysis,
            >::default())),
            bases: Default::default(),
            bases_walked: Default::default(),
        }
    }

    /// The walks down a pointer's GEP chain made so far.
    pub fn bases_walked(&self) -> u64 {
        self.bases_walked.get()
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
        // A def whose writes the lists do not name is asked of BasicAA.
        let found = if writes.is_none() {
            self.unknown_call_mod_ref(def, loc).writes
        } else {
            memoryssa::changes(&loc.effective, false, writes, |written| {
                self.may_overlap(&loc.effective, written, Some(def))
            })
        };
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
            Some(list) => list.iter().any(|one| writes.iter().any(|w| self.may_overlap(one, w, Some(def)))),
        };
        touched(self.accesses.reads(call)) || touched(self.accesses.writes(call))
    }

    /// The object `reference` is rooted at: the root of an object's own
    /// address, else what its pointer is derived from through geps and casts
    /// (`getUnderlyingObject`).
    fn base_of(
        &self,
        reference: &MemRef,
    ) -> Option<llrm_mir::module::Operand> {
        if reference.object {
            reference.root
        } else {
            let unit = self.unit;
            let pointer = reference.pointer?;
            if let Some(&base) = self.bases.borrow().get(&pointer) {
                return Some(base);
            }
            self.bases_walked.set(self.bases_walked.get() + 1);
            let base = llrm_mir::valuetracking::underlying(unit.context, unit.layout, unit.function, pointer).0;
            self.bases.borrow_mut().insert(pointer, base);
            Some(base)
        }
    }

    /// The identified object (a global or an alloca) `reference` points into:
    /// `getUnderlyingObject` and `isIdentifiedObject`.
    fn identified_object(
        &self,
        reference: &MemRef,
    ) -> Option<llrm_mir::module::Operand> {
        use llrm_mir::context::ConstantKind;
        use llrm_mir::module::{Operand, ValueDef};
        let base = self.base_of(reference)?;
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

    /// What a call whose accesses the lists do not name may do to `loc`:
    /// `getModRefInfo(Call, Loc)`, which reads the call's attributes, the
    /// arguments that may alias the location and whether the location's object
    /// is captured before the call. Anything where it has no pointer.
    fn unknown_call_mod_ref(
        &self,
        call: InstId,
        loc: &Location,
    ) -> ModRef {
        use crate::aa::{AAResults, LocationSize, MemoryLocation};
        let anything = ModRef { reads: true, writes: true };
        let (Opcode::Call(_) | Opcode::Invoke(_)) = self.unit.function.instruction(call).opcode else {
            return anything;
        };
        let Some(pointer) = loc.reference.pointer else { return anything };
        let size = if loc.effective.width >= UNBOUNDED_WIDTH {
            LocationSize::BeforeOrAfterPointer
        } else {
            LocationSize::Precise(u64::from(loc.effective.width))
        };
        let found = crate::basicaa::BasicAA::new(self.unit).get_mod_ref_info(
            call,
            Some(&MemoryLocation::new(pointer, size)),
            &mut self.basic.borrow_mut(),
        );
        ModRef { reads: found.is_ref_set(), writes: found.is_mod_set() }
    }

    /// Whether the address of the alloca `alloca` may be captured before `at`
    /// (`EarliestEscapeInfo`; anywhere, a return included, where `at` is none),
    /// remembered in the batch.
    fn captured(
        &self,
        alloca: llrm_mir::module::ValueId,
        at: Option<InstId>,
    ) -> bool {
        let basic = crate::basicaa::BasicAA::new(self.unit);
        let object = llrm_mir::module::Operand::Value(alloca);
        match at {
            Some(at) => basic.is_captured_before(object, Some(at), &mut self.basic.borrow_mut()),
            None => basic.is_captured(object, &mut self.basic.borrow_mut()),
        }
    }

    /// Whether `pointer` is derived only from objects other than the alloca
    /// `alloca`, whose address is never exposed: a pointer loaded from memory,
    /// an argument, a call's result or an integer cannot hold that address, and
    /// a phi or select holds it only if one of its incoming pointers does.
    fn derived_from_elsewhere(
        &self,
        pointer: llrm_mir::module::Operand,
        alloca: llrm_mir::module::ValueId,
        depth: u32,
    ) -> bool {
        use llrm_mir::module::{Operand, ValueDef};
        use llrm_mir::opcode::CastOp;
        let unit = self.unit;
        let (base, _) = llrm_mir::valuetracking::underlying(unit.context, unit.layout, unit.function, pointer);
        let Operand::Value(value) = base else { return true };
        let def = match unit.function.value(value).def {
            ValueDef::Argument(_) => return true,
            ValueDef::Instruction(def) => def,
        };
        let instruction = unit.function.instruction(def);
        match &instruction.opcode {
            Opcode::Load { .. } | Opcode::Call(_) | Opcode::Cast(CastOp::IntToPtr) => true,
            Opcode::Alloca { .. } => value != alloca,
            Opcode::Phi if depth < 6 => {
                instruction.operands.chunks(2).all(|pair| self.derived_from_elsewhere(pair[0], alloca, depth + 1))
            }
            Opcode::Select if depth < 6 => {
                instruction.operands[1..].iter().all(|&one| self.derived_from_elsewhere(one, alloca, depth + 1))
            }
            _ => false,
        }
    }

    /// Whether `local` is an alloca whose address is never exposed and `other`
    /// is a pointer from elsewhere: nothing outside the function can hold the
    /// alloca's address, so the two cannot overlap. The stand-in for capture
    /// tracking, asked of the whole function.
    fn unreachable_from(
        &self,
        local: &MemRef,
        other: &MemRef,
        at: Option<InstId>,
    ) -> bool {
        use llrm_mir::module::Operand;
        let Some(Operand::Value(alloca)) = self.identified_object(local) else { return false };
        if self.captured(alloca, at) {
            return false;
        }
        match other.pointer {
            Some(pointer) => self.derived_from_elsewhere(pointer, alloca, 0),
            // Memory no pointer of the function names (a call's unknown or
            // non-local reads): the alloca's address is not among what it
            // is reached through.
            None => other.provenance.as_ref().is_some_and(|provenance| {
                provenance
                    .slices
                    .iter()
                    .all(
                        |slice| matches!(
                            slice.object.kind,
                            crate::memory::MemoryKind::Unknown
                                | crate::memory::MemoryKind::Nonlocal
                                | crate::memory::MemoryKind::External
                        ),
                    )
            }),
        }
    }

    /// `AliasAnalysis::alias` of two references, as far as the stand-in goes:
    /// two identified objects that differ do not overlap (BasicAA), else the
    /// overlap test of `may_clobber`.
    fn may_overlap(
        &self,
        one: &MemRef,
        other: &MemRef,
        at: Option<InstId>,
    ) -> bool {
        if one.space == other.space
            && let (Some(left), Some(right)) = (self.identified_object(one), self.identified_object(other))
            && left != right
        {
            return false;
        }
        if self.unreachable_from(one, other, at) || self.unreachable_from(other, one, at) {
            return false;
        }
        // The cheap tests first: BasicAA is asked only of what they leave.
        memoryssa::may_clobber(self.unit, None, one, other) && !self.basic_no_alias(one, other)
    }

    /// `BasicAA` says the two references, each a pointer and a size, do not
    /// alias.
    fn basic_no_alias(
        &self,
        one: &MemRef,
        other: &MemRef,
    ) -> bool {
        use crate::aa::{AAResults, LocationSize, MemoryLocation};
        let (Some(left), Some(right)) = (one.pointer, other.pointer) else { return false };
        let size = |reference: &MemRef| {
            if reference.width >= UNBOUNDED_WIDTH {
                LocationSize::BeforeOrAfterPointer
            } else {
                LocationSize::Precise(u64::from(reference.width))
            }
        };
        let (left, right) = (MemoryLocation::new(left, size(one)), MemoryLocation::new(right, size(other)));
        crate::basicaa::BasicAA::new(self.unit).alias(&left, &right, &mut self.basic.borrow_mut(), None)
            == crate::aa::AliasResult::NoAlias
    }

    /// `MemoryLocation::getWithoutAATags()`: the location with no `!tbaa` type.
    pub fn without_aa_tags(
        &self,
        loc: &Rc<Location>,
    ) -> Rc<Location> {
        if loc.reference.typed.is_none() {
            return Rc::clone(loc);
        }
        let reference = MemRef { typed: None, lineage: Rc::from(Vec::new()), ..loc.reference.clone() };
        self.location_for(&reference, loc.unbounded)
    }

    /// `AA.alias(A, B)`, as far as the stand-in goes: no alias where the bytes
    /// cannot overlap; must alias where both name the same bytes by the same
    /// address; partial where the address is the same and the sizes differ.
    pub fn alias(
        &self,
        one: &Location,
        other: &Location,
    ) -> AliasResult {
        if !self.may_overlap(&one.effective, &other.effective, None) {
            return AliasResult::No;
        }
        let (a, b) = (&one.reference, &other.reference);
        let same_address = a.pointer.is_some()
            && a.root == b.root
            && a.disp == b.disp
            && a.base == b.base
            && a.scale == b.scale
            && a.base_width == b.base_width
            && a.segment == b.segment
            && a.selector == b.selector
            && a.origin == b.origin
            && a.linear == b.linear
            && a.object == b.object
            && a.space == b.space;
        if same_address && !one.unbounded && !other.unbounded {
            return if a.width == b.width { AliasResult::Must } else { AliasResult::Partial };
        }
        AliasResult::May
    }

    /// `AA.getModRefInfo(I, Loc)` for a load, a store or a call: whether `inst`
    /// may read and may write a byte of `loc`.
    pub fn get_mod_ref_info(
        &self,
        inst: InstId,
        loc: &Location,
    ) -> ModRef {
        let touches = |list: Option<&[MemRef]>| {
            list.map(|list| list.iter().any(|one| self.may_overlap(one, &loc.effective, Some(inst))))
        };
        let (reads, writes) = (touches(self.accesses.reads(inst)), touches(self.accesses.writes(inst)));
        if reads.is_some() && writes.is_some() {
            return ModRef { reads: reads.unwrap_or(false), writes: writes.unwrap_or(false) };
        }
        let unknown = self.unknown_call_mod_ref(inst, loc);
        ModRef { reads: reads.unwrap_or(unknown.reads), writes: writes.unwrap_or(unknown.writes) }
    }

    /// `AA.getMemoryEffects(CB)`: whether the call may read and may write
    /// memory.
    pub fn call_effects(
        &self,
        call: InstId,
    ) -> (bool, bool) {
        // Memory no pointer reaches (`inaccessiblemem`) is no location of the
        // access lists, yet a call that reads or writes it is not one that
        // `doesNotAccessMemory`: a BIOS poll in a loop is not loop-invariant.
        let hidden = self.inaccessible_effects(call);
        (
            hidden.reads || self.accesses.reads(call).is_none_or(|reads| !reads.is_empty()),
            hidden.writes || self.accesses.writes(call).is_none_or(|writes| !writes.is_empty()),
        )
    }

    /// What the call's `memory(inaccessiblemem: ...)`, at the site and on its
    /// callee, allows.
    fn inaccessible_effects(
        &self,
        call: InstId,
    ) -> llrm_mir::memory::Effects {
        let unit = self.unit;
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &unit.function.instruction(call).opcode else {
            return llrm_mir::memory::Effects::NONE;
        };
        let callee = llrm_mir::memory::callee(unit.context, unit.function, call)
            .and_then(|one| unit.globals.get(one.0 as usize))
            .and_then(|one| one.function())
            .map(|one| one.attrs.as_slice());
        // An attribute states a bound; a callee with none is known by its body
        // alone, which the access lists already hold.
        let states = |attrs: &[llrm_mir::opcode::Attribute]| {
            attrs
                .iter()
                .any(|one| matches!(
                    one,
                    llrm_mir::opcode::Attribute::Memory(_) | llrm_mir::opcode::Attribute::Flag(_)
                ))
        };
        if !states(&info.attrs) && !callee.is_some_and(states) {
            return llrm_mir::memory::Effects::NONE;
        }
        // Each bounds what the call does there: the narrower holds.
        let site = llrm_mir::memory::inaccessible(&info.attrs);
        let declared = callee.map_or(site, llrm_mir::memory::inaccessible);
        llrm_mir::memory::Effects { reads: site.reads && declared.reads, writes: site.writes && declared.writes }
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
        first_touched.iter().any(|one| second_touched.iter().any(|other| self.may_overlap(one, other, Some(first))))
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
