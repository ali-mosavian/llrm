//! `AliasSetTracker` (LLVM `AliasSetTracker.cpp`, `d1106deb71cc`): the
//! locations an analysis asks of, grouped into sets that may alias, with
//! whether each set's locations all must alias. Over the locations of
//! `mssa::alias::AliasQuery`.
//!
//! DEVIATE: the reference counts and the intrusive list are not kept: a set
//! that forwards is skipped by every iteration, and one that does not is
//! referenced by the pointer map, so no set is ever removed. Unknown
//! instructions (`addUnknown`) are not tracked: LICM adds loads and stores
//! only.
//!
//! # Example
//! Two stores through pointers that may alias share a set; through pointers
//! that cannot alias they are two sets:
//!
//! ```text
//! store i32 1, ptr %a
//! store i32 2, ptr %b   ; one set if %a and %b may alias, two if they do not
//! ```

use std::rc::Rc;

use llrm_mir::module::{InstId, Operand};

use crate::mssa::alias::{AliasQuery, AliasResult, Location, ModRef};

/// What the sets read of a location.
pub trait AliasLocation {
    /// The pointer value of the location.
    fn pointer(&self) -> Option<Operand>;
    /// Whether `other` is this very location (same pointer, same extent).
    fn is_same(
        &self,
        other: &Self,
    ) -> bool;
}

/// What the tracker asks of the alias analysis over its locations: MemorySSA's
/// [`AliasQuery`], or a `BatchAAResults` (`LoopAccessAnalysis`).
pub trait AliasLocations {
    type Loc: AliasLocation;

    /// `alias(A, B)`.
    fn alias(
        &self,
        one: &Self::Loc,
        other: &Self::Loc,
    ) -> AliasResult;

    /// `getModRefInfo(Inst, Loc)`.
    fn get_mod_ref_info(
        &self,
        inst: InstId,
        member: &Self::Loc,
    ) -> ModRef;

    /// The location with its AA tags dropped.
    fn without_aa_tags(
        &self,
        location: &Rc<Self::Loc>,
    ) -> Rc<Self::Loc>;
}

impl AliasLocation for Location {
    fn pointer(&self) -> Option<Operand> {
        self.reference.pointer
    }

    fn is_same(
        &self,
        other: &Self,
    ) -> bool {
        self.reference == other.reference && self.unbounded == other.unbounded
    }
}

impl<'b> AliasLocations for AliasQuery<'b> {
    type Loc = Location;

    fn alias(
        &self,
        one: &Location,
        other: &Location,
    ) -> AliasResult {
        AliasQuery::alias(self, one, other)
    }

    fn get_mod_ref_info(
        &self,
        inst: InstId,
        member: &Location,
    ) -> ModRef {
        AliasQuery::get_mod_ref_info(self, inst, member)
    }

    fn without_aa_tags(
        &self,
        location: &Rc<Location>,
    ) -> Rc<Location> {
        AliasQuery::without_aa_tags(self, location)
    }
}

/// `-alias-set-saturation-threshold`: the most locations the sets hold before
/// they are lumped into one.
pub const SATURATION_THRESHOLD: usize = 250;

/// [`AliasSet`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/AliasSetTracker.h#L49).
#[derive(Debug)]
pub struct AliasSet<L = Location> {
    /// The set this one was merged into.
    forward: Option<usize>,
    /// `MemoryLocs`.
    locations: Vec<Rc<L>>,
    /// `AliasAny`: the set is to be considered to alias any pointer.
    alias_any: bool,
    /// `Alias == SetMustAlias`.
    must_alias: bool,
    /// `Access`.
    access: ModRef,
}

impl<L: AliasLocation> AliasSet<L> {
    fn new() -> Self {
        Self { forward: None, locations: Vec::new(), alias_any: false, must_alias: true, access: ModRef::NONE }
    }

    pub fn is_ref(&self) -> bool {
        self.access.is_ref()
    }

    pub fn is_mod(&self) -> bool {
        self.access.is_mod()
    }

    pub fn is_must_alias(&self) -> bool {
        self.must_alias
    }

    pub fn is_may_alias(&self) -> bool {
        !self.must_alias
    }

    /// `isForwardingAliasSet`.
    pub fn is_forwarding_alias_set(&self) -> bool {
        self.forward.is_some()
    }

    pub fn locations(&self) -> &[Rc<L>] {
        &self.locations
    }

    /// `getPointers`: the pointer values of the locations, each once, in order.
    pub fn get_pointers(&self) -> Vec<Operand> {
        let mut found: Vec<Operand> = Vec::new();
        for location in &self.locations {
            if let Some(pointer) = location.pointer()
                && !found.contains(&pointer)
            {
                found.push(pointer);
            }
        }
        found
    }
}

/// [`AliasSetTracker`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/AliasSetTracker.h#L201).
pub struct AliasSetTracker<'a, Q: AliasLocations> {
    aa: &'a Q,
    sets: Vec<AliasSet<Q::Loc>>,
    /// `PointerMap`: the set holding the locations of a pointer value.
    pointer_map: llrm_support::hash::HashMap<Operand, usize>,
    /// `TotalAliasSetSize`.
    total_alias_set_size: usize,
    /// `AliasAnyAS`: set once the tracker is saturated.
    alias_any_as: Option<usize>,
}

impl<'a, Q: AliasLocations> AliasSetTracker<'a, Q> {
    pub fn new(aa: &'a Q) -> Self {
        Self { aa, sets: Vec::new(), pointer_map: Default::default(), total_alias_set_size: 0, alias_any_as: None }
    }

    /// The sets that do not forward, in the order they were made.
    pub fn alias_sets(&self) -> impl Iterator<Item = (usize, &AliasSet<Q::Loc>)> {
        self.sets.iter().enumerate().filter(|(_, set)| set.forward.is_none())
    }

    pub fn alias_set(
        &self,
        at: usize,
    ) -> &AliasSet<Q::Loc> {
        &self.sets[at]
    }

    /// `collapseForwardingIn`: the set `at` has been merged into, if it has.
    fn collapse_forwarding(
        &mut self,
        at: usize,
    ) -> usize {
        let mut last = at;
        while let Some(next) = self.sets[last].forward {
            last = next;
        }
        // The links on the way point at the end of the chain.
        let mut walk = at;
        while let Some(next) = self.sets[walk].forward {
            self.sets[walk].forward = Some(last);
            walk = next;
            if walk == last {
                break;
            }
        }
        last
    }

    /// [`AliasSet::mergeSetIn`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/AliasSetTracker.cpp#L42):
    /// the set `from` into the set `into`.
    fn merge_set_in(
        &mut self,
        into: usize,
        from: usize,
    ) {
        assert!(self.sets[from].forward.is_none() && self.sets[into].forward.is_none());
        // Update the alias and access types of this set...
        let from_access = self.sets[from].access;
        self.sets[into].access = self.sets[into].access.union(from_access);
        self.sets[into].must_alias &= self.sets[from].must_alias;
        if self.sets[into].must_alias {
            // Check that these two merged sets really are must aliases. If we
            // cannot find a must-alias pair between them, this set becomes a
            // may alias.
            let any_must = self
                .sets[into]
                .locations
                .iter()
                .any(
                    |one| self.sets[from].locations.iter().any(|other| self.aa.alias(one, other) == AliasResult::Must),
                );
            if !any_must {
                self.sets[into].must_alias = false;
            }
        }
        // Merge the list of constituent memory locations...
        let moved = std::mem::take(&mut self.sets[from].locations);
        self.sets[into].locations.extend(moved);
        self.sets[from].forward = Some(into);
    }

    /// [`AliasSet::aliasesMemoryLocation`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/AliasSetTracker.cpp#L128).
    fn aliases_memory_location(
        &self,
        set: usize,
        location: &Q::Loc,
    ) -> AliasResult {
        let set = &self.sets[set];
        if set.alias_any {
            return AliasResult::May;
        }
        // Check all of the memory locations in the set...
        for member in &set.locations {
            let result = self.aa.alias(location, member);
            if result != AliasResult::No {
                return result;
            }
        }
        AliasResult::No
    }

    /// [`AliasSet::aliasesUnknownInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/AliasSetTracker.cpp#L150)
    /// for an instruction that is not a call (the set holds no unknown
    /// instruction): how `inst` may touch the locations of the set.
    pub fn aliases_unknown_inst(
        &self,
        set: usize,
        inst: InstId,
        touches_memory: bool,
    ) -> ModRef {
        let set = &self.sets[set];
        if set.alias_any {
            return ModRef { reads: true, writes: true };
        }
        if !touches_memory {
            return ModRef::NONE;
        }
        let mut found = ModRef::NONE;
        for member in &set.locations {
            found = found.union(self.aa.get_mod_ref_info(inst, member));
            if found.is_mod_and_ref() {
                return found;
            }
        }
        found
    }

    /// [`mergeAliasSetsForMemoryLocation`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/AliasSetTracker.cpp#L219):
    /// the set that results from merging every set that may alias `location`,
    /// and whether it must alias all of them.
    fn merge_alias_sets_for_memory_location(
        &mut self,
        location: &Q::Loc,
        ptr_as: Option<usize>,
    ) -> (Option<usize>, bool) {
        let mut found: Option<usize> = None;
        let mut must_alias_all = true;
        for at in 0..self.sets.len() {
            if self.sets[at].forward.is_some() {
                continue;
            }
            // An alias set that already contains a memory location with the
            // same pointer value is directly assumed to MustAlias; we bypass
            // the AA query in this case.
            if Some(at) != ptr_as {
                let result = self.aliases_memory_location(at, location);
                if result == AliasResult::No {
                    continue;
                }
                if result != AliasResult::Must {
                    must_alias_all = false;
                }
            }
            match found {
                // If this is the first alias set ptr can go into, remember it.
                None => found = Some(at),
                // Otherwise, we must merge the sets.
                Some(first) => self.merge_set_in(first, at),
            }
        }
        (found, must_alias_all)
    }

    /// [`getAliasSetFor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/AliasSetTracker.cpp#L278):
    /// the set holding `location`, made and filled when there is none.
    pub fn get_alias_set_for(
        &mut self,
        location: &Rc<Q::Loc>,
    ) -> usize {
        // The alias sets are indexed with a map from the memory locations'
        // pointer values. If the memory location is already registered, we can
        // find it in the alias set associated with its pointer.
        let pointer = location.pointer();
        let mut entry = pointer.and_then(|pointer| self.pointer_map.get(&pointer).copied());
        if let Some(found) = entry {
            let found = self.collapse_forwarding(found);
            entry = Some(found);
            if self.sets[found].locations.iter().any(|one| one.is_same(location)) {
                return found;
            }
        }
        let (set, must_alias_all) = if let Some(any) = self.alias_any_as {
            // At this point, the AST is saturated, so we only have one active
            // alias set.
            (any, false)
        } else if let (Some(merged), must_all) = self.merge_alias_sets_for_memory_location(location, entry) {
            (merged, must_all)
        } else {
            // Otherwise create a new alias set to hold the new memory location.
            self.sets.push(AliasSet::new());
            (self.sets.len() - 1, true)
        };
        // Register memory location in selected alias set.
        self.add_memory_location_to_set(set, location, must_alias_all);
        // Register selected alias set in pointer map (or ensure it is
        // consistent with earlier map entry after taking into account new
        // merging).
        if let Some(pointer) = pointer {
            match entry {
                Some(found) => {
                    let found = self.collapse_forwarding(found);
                    assert_eq!(found, set, "memory locations with the same pointer value are in different alias sets");
                }
                None => {
                    self.pointer_map.insert(pointer, set);
                }
            }
        }
        set
    }

    /// `AliasSet::addMemoryLocation`.
    fn add_memory_location_to_set(
        &mut self,
        set: usize,
        location: &Rc<Q::Loc>,
        known_must_alias: bool,
    ) {
        if self.sets[set].must_alias && !known_must_alias {
            // If we cannot find a must-alias with any of the existing
            // MemoryLocs, we must downgrade to may-alias.
            let any_must =
                self.sets[set].locations.iter().any(|member| self.aa.alias(location, member) == AliasResult::Must);
            if !any_must {
                self.sets[set].must_alias = false;
            }
        }
        // Add it to the end of the list...
        self.sets[set].locations.push(Rc::clone(location));
        self.total_alias_set_size += 1;
    }

    /// [`addMemoryLocation`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/AliasSetTracker.cpp#L466):
    /// `location` with the access `access`; the tracker saturates past the
    /// threshold.
    pub fn add_memory_location(
        &mut self,
        location: &Rc<Q::Loc>,
        access: ModRef,
    ) -> usize {
        let set = self.get_alias_set_for(location);
        self.sets[set].access = self.sets[set].access.union(access);
        if self.alias_any_as.is_none() && self.total_alias_set_size > SATURATION_THRESHOLD {
            // The AST is now saturated. From here on, we conservatively
            // consider all elements to alias each-other.
            return self.merge_all_alias_sets();
        }
        set
    }

    /// `add(LoadInst *)`.
    pub fn add_load(
        &mut self,
        location: &Rc<Q::Loc>,
    ) {
        self.add_memory_location(location, ModRef::REF);
    }

    /// `add(StoreInst *)`.
    pub fn add_store(
        &mut self,
        location: &Rc<Q::Loc>,
    ) {
        self.add_memory_location(location, ModRef::MOD);
    }

    /// `addWithoutAATags(StoreInst *)`.
    pub fn add_store_without_aa_tags(
        &mut self,
        location: &Rc<Q::Loc>,
    ) {
        let bare = self.aa.without_aa_tags(location);
        self.add_memory_location(&bare, ModRef::MOD);
    }

    /// [`mergeAllAliasSets`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/AliasSetTracker.cpp#L424).
    fn merge_all_alias_sets(&mut self) -> usize {
        assert!(self.alias_any_as.is_none() && self.total_alias_set_size > SATURATION_THRESHOLD);
        let live: Vec<usize> = (0..self.sets.len()).filter(|&at| self.sets[at].forward.is_none()).collect();
        // Copy all instructions and memory locations into a new set, and
        // forward all other sets to it.
        self.sets.push(AliasSet::new());
        let any = self.sets.len() - 1;
        self.alias_any_as = Some(any);
        self.sets[any].must_alias = false;
        self.sets[any].access = ModRef { reads: true, writes: true };
        self.sets[any].alias_any = true;
        for at in live {
            // Otherwise, perform the actual merge.
            let moved = std::mem::take(&mut self.sets[at].locations);
            self.sets[any].locations.extend(moved);
            self.sets[at].forward = Some(any);
        }
        any
    }
}
