//! The interface of alias analysis, as LLVM's `AAResults` and `BatchAAResults`
//! (llvm/Analysis/AliasAnalysis.h at d1106deb71cc): the queries a client asks
//! (MemorySSA, MemDep, LICM, GVN, DSE), not an implementation. Names are
//! LLVM's in snake case; the primary queries are `alias`,
//! `get_mod_ref_info_mask`, `get_memory_effects`, `get_arg_mod_ref_info` and
//! the `get_mod_ref_info*` forms; every other query is a provided method over
//! those, as in LLVM.
//!
//! Nothing here keeps a fact about the whole function: a query is answered
//! from the uses and definitions it reaches, and what one query learns is kept
//! in the `AAQueryInfo` of its batch and dropped with it, never shared between
//! two versions of a body.

use std::collections::HashMap;

use llrm_mir::module::{InstId, MetadataId, Operand};

/// How much of memory an access touches (`LocationSize`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LocationSize {
    /// Exactly `n` bytes from the pointer.
    Precise(u64),
    /// At most `n` bytes from the pointer, at an unknown place.
    UpperBound(u64),
    /// Any bytes before or after the pointer, within its object.
    BeforeOrAfterPointer,
    Unknown,
}

/// Metadata an access carries that alias analysis reads (`AAMDNodes`).
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct AAInfo {
    pub tbaa: Option<MetadataId>,
    pub noalias_scope: Option<MetadataId>,
}

/// A place in memory: a pointer and how much of it (`MemoryLocation`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MemoryLocation {
    pub ptr: Operand,
    pub size: LocationSize,
    pub aa_info: AAInfo,
}

impl MemoryLocation {
    pub fn new(
        ptr: Operand,
        size: LocationSize,
    ) -> Self {
        Self { ptr, size, aa_info: AAInfo::default() }
    }

    /// `MemoryLocation::getBeforeOrAfter`.
    pub fn before_or_after(ptr: Operand) -> Self {
        Self::new(ptr, LocationSize::BeforeOrAfterPointer)
    }

    pub fn with_new_ptr(
        self,
        ptr: Operand,
    ) -> Self {
        Self { ptr, ..self }
    }

    pub fn with_new_size(
        self,
        size: LocationSize,
    ) -> Self {
        Self { size, ..self }
    }
}

/// `AliasResult`: the offset of the first location from the second is kept for
/// a partial alias, as LLVM keeps it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AliasResult {
    NoAlias,
    MayAlias,
    PartialAlias { offset: Option<i32> },
    MustAlias,
}

/// `ModRefInfo`: what an access may do to a location.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ModRefInfo {
    NoModRef,
    Ref,
    Mod,
    ModRef,
}

impl ModRefInfo {
    pub fn is_no_mod_ref(self) -> bool {
        self == Self::NoModRef
    }

    pub fn is_mod_set(self) -> bool {
        matches!(self, Self::Mod | Self::ModRef)
    }

    pub fn is_ref_set(self) -> bool {
        matches!(self, Self::Ref | Self::ModRef)
    }

    /// `intersectModRef`.
    pub fn intersect(
        self,
        other: Self,
    ) -> Self {
        Self::from_bits(self.bits() & other.bits())
    }

    /// `unionModRef`.
    pub fn union(
        self,
        other: Self,
    ) -> Self {
        Self::from_bits(self.bits() | other.bits())
    }

    fn bits(self) -> u8 {
        match self {
            Self::NoModRef => 0,
            Self::Ref => 1,
            Self::Mod => 2,
            Self::ModRef => 3,
        }
    }

    fn from_bits(bits: u8) -> Self {
        match bits & 3 {
            0 => Self::NoModRef,
            1 => Self::Ref,
            2 => Self::Mod,
            _ => Self::ModRef,
        }
    }
}

/// The places a call may touch (`IRMemLocation`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IRMemLocation {
    ArgMem,
    InaccessibleMem,
    ErrnoMem,
    Other,
}

/// What a call does to each kind of memory (`MemoryEffects`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MemoryEffects {
    effects: [ModRefInfo; 4],
}

impl MemoryEffects {
    pub fn unknown() -> Self {
        Self { effects: [ModRefInfo::ModRef; 4] }
    }

    pub fn none() -> Self {
        Self { effects: [ModRefInfo::NoModRef; 4] }
    }

    pub fn mod_ref(
        self,
        at: IRMemLocation,
    ) -> ModRefInfo {
        self.effects[at as usize]
    }

    pub fn with(
        mut self,
        at: IRMemLocation,
        info: ModRefInfo,
    ) -> Self {
        self.effects[at as usize] = info;
        self
    }

    /// The union over every location (`getModRef()`).
    pub fn overall(self) -> ModRefInfo {
        self.effects.into_iter().fold(ModRefInfo::NoModRef, ModRefInfo::union)
    }

    pub fn does_not_access_memory(self) -> bool {
        self.overall().is_no_mod_ref()
    }

    pub fn only_reads_memory(self) -> bool {
        !self.overall().is_mod_set()
    }

    pub fn only_accesses_arg_memory(self) -> bool {
        [IRMemLocation::InaccessibleMem, IRMemLocation::ErrnoMem, IRMemLocation::Other]
            .into_iter()
            .all(|at| self.mod_ref(at).is_no_mod_ref())
    }
}

/// What the queries of one batch keep (`AAQueryInfo`): the answers asked for
/// pairs, and the two switches a walk sets.
#[derive(Debug)]
pub struct AAQueryInfo {
    /// The query may be asked about different iterations of a loop
    /// (`MayBeCrossIteration`).
    pub may_be_cross_iteration: bool,
    /// Whether the dominator tree may be used (`UseDominatorTree`).
    pub use_dominator_tree: bool,
    pub cache: HashMap<(MemoryLocation, MemoryLocation), AliasResult>,
}

impl Default for AAQueryInfo {
    fn default() -> Self {
        Self { may_be_cross_iteration: false, use_dominator_tree: true, cache: HashMap::new() }
    }
}

/// The queries (`AAResults`). An implementation answers for one body as it
/// stands: it is made for one version of a function and dropped with it.
pub trait AAResults {
    /// `alias(LocA, LocB, AAQI, CtxI)`: `ctx` is the instruction the query is
    /// asked at, or none for the whole function.
    fn alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        ctx: Option<InstId>,
    ) -> AliasResult;

    /// `getModRefInfoMask`: what may be applied to every access to `loc`; no
    /// Mod where it is constant, no Mod or Ref for a local with
    /// `ignore_locals`.
    fn get_mod_ref_info_mask(
        &mut self,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        ignore_locals: bool,
    ) -> ModRefInfo;

    /// `getArgModRefInfo`.
    fn get_arg_mod_ref_info(
        &mut self,
        call: InstId,
        arg: u32,
    ) -> ModRefInfo;

    /// `getMemoryEffects(Call)`.
    fn get_memory_effects(
        &mut self,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> MemoryEffects;

    /// `getModRefInfo(Instruction, optional<MemoryLocation>)`: with none, what
    /// the instruction does to memory at all (a Use, a Def or neither).
    fn get_mod_ref_info(
        &mut self,
        inst: InstId,
        loc: Option<&MemoryLocation>,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo;

    /// `getModRefInfo(Instruction, CallBase)`: what `inst` does to what `call`
    /// touches.
    fn get_mod_ref_info_call(
        &mut self,
        inst: InstId,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo;

    /// `callCapturesBefore`: what `inst` may do to `loc` through a call that
    /// captured its pointer.
    fn call_captures_before(
        &mut self,
        inst: InstId,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo;

    // Provided, as LLVM's.

    fn is_no_alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
    ) -> bool {
        self.alias(a, b, aaqi, None) == AliasResult::NoAlias
    }

    fn is_must_alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
    ) -> bool {
        self.alias(a, b, aaqi, None) == AliasResult::MustAlias
    }

    /// `pointsToConstantMemory`: a wrapper over `get_mod_ref_info_mask`.
    fn points_to_constant_memory(
        &mut self,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        or_local: bool,
    ) -> bool {
        self.get_mod_ref_info_mask(loc, aaqi, or_local).is_no_mod_ref()
    }

    fn does_not_access_memory(
        &mut self,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> bool {
        self.get_memory_effects(call, aaqi).does_not_access_memory()
    }

    fn only_reads_memory(
        &mut self,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> bool {
        self.get_memory_effects(call, aaqi).only_reads_memory()
    }
}

/// `BatchAAResults`: an `AAResults` and the one `AAQueryInfo` its queries
/// share, for a walk that edits nothing between queries. To leave the batch,
/// make another.
pub struct BatchAAResults<'a, A: AAResults> {
    aa: &'a mut A,
    aaqi: AAQueryInfo,
}

impl<'a, A: AAResults> BatchAAResults<'a, A> {
    pub fn new(aa: &'a mut A) -> Self {
        Self { aa, aaqi: AAQueryInfo::default() }
    }

    pub fn alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
    ) -> AliasResult {
        self.aa.alias(a, b, &mut self.aaqi, None)
    }

    pub fn is_no_alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
    ) -> bool {
        self.alias(a, b) == AliasResult::NoAlias
    }

    pub fn is_must_alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
    ) -> bool {
        self.alias(a, b) == AliasResult::MustAlias
    }

    pub fn get_mod_ref_info_mask(
        &mut self,
        loc: &MemoryLocation,
        ignore_locals: bool,
    ) -> ModRefInfo {
        self.aa.get_mod_ref_info_mask(loc, &mut self.aaqi, ignore_locals)
    }

    pub fn points_to_constant_memory(
        &mut self,
        loc: &MemoryLocation,
        or_local: bool,
    ) -> bool {
        self.get_mod_ref_info_mask(loc, or_local).is_no_mod_ref()
    }

    pub fn get_mod_ref_info(
        &mut self,
        inst: InstId,
        loc: Option<&MemoryLocation>,
    ) -> ModRefInfo {
        self.aa.get_mod_ref_info(inst, loc, &mut self.aaqi)
    }

    pub fn get_mod_ref_info_call(
        &mut self,
        inst: InstId,
        call: InstId,
    ) -> ModRefInfo {
        self.aa.get_mod_ref_info_call(inst, call, &mut self.aaqi)
    }

    pub fn get_arg_mod_ref_info(
        &mut self,
        call: InstId,
        arg: u32,
    ) -> ModRefInfo {
        self.aa.get_arg_mod_ref_info(call, arg)
    }

    pub fn get_memory_effects(
        &mut self,
        call: InstId,
    ) -> MemoryEffects {
        self.aa.get_memory_effects(call, &mut self.aaqi)
    }

    pub fn call_captures_before(
        &mut self,
        inst: InstId,
        loc: &MemoryLocation,
    ) -> ModRefInfo {
        self.aa.call_captures_before(inst, loc, &mut self.aaqi)
    }

    pub fn enable_cross_iteration_mode(&mut self) {
        self.aaqi.may_be_cross_iteration = true;
    }

    pub fn disable_dominator_tree(&mut self) {
        self.aaqi.use_dominator_tree = false;
    }

    /// `BatchAACrossIterationScope`: runs `query` with the cross-iteration
    /// switch set to `cross_iteration`, and restores it after.
    pub fn cross_iteration_scope<R>(
        &mut self,
        cross_iteration: bool,
        query: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let was = std::mem::replace(&mut self.aaqi.may_be_cross_iteration, cross_iteration);
        let out = query(self);
        self.aaqi.may_be_cross_iteration = was;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An analysis that answers nothing: every pair may alias, every access
    /// does both.
    struct Nothing;

    impl AAResults for Nothing {
        fn alias(
            &mut self,
            _: &MemoryLocation,
            _: &MemoryLocation,
            _: &mut AAQueryInfo,
            _: Option<InstId>,
        ) -> AliasResult {
            AliasResult::MayAlias
        }

        fn get_mod_ref_info_mask(
            &mut self,
            _: &MemoryLocation,
            _: &mut AAQueryInfo,
            _: bool,
        ) -> ModRefInfo {
            ModRefInfo::ModRef
        }

        fn get_arg_mod_ref_info(
            &mut self,
            _: InstId,
            _: u32,
        ) -> ModRefInfo {
            ModRefInfo::ModRef
        }

        fn get_memory_effects(
            &mut self,
            _: InstId,
            _: &mut AAQueryInfo,
        ) -> MemoryEffects {
            MemoryEffects::unknown()
        }

        fn get_mod_ref_info(
            &mut self,
            _: InstId,
            _: Option<&MemoryLocation>,
            _: &mut AAQueryInfo,
        ) -> ModRefInfo {
            ModRefInfo::ModRef
        }

        fn get_mod_ref_info_call(
            &mut self,
            _: InstId,
            _: InstId,
            _: &mut AAQueryInfo,
        ) -> ModRefInfo {
            ModRefInfo::ModRef
        }

        fn call_captures_before(
            &mut self,
            _: InstId,
            _: &MemoryLocation,
            _: &mut AAQueryInfo,
        ) -> ModRefInfo {
            ModRefInfo::ModRef
        }
    }

    /// The provided methods are the primary ones asked:
    /// `points_to_constant_memory` is the mask, and a batch restores the
    /// cross-iteration switch after a scope.
    #[test]
    fn provided_queries_are_wrappers_of_the_primary_ones() {
        let mut nothing = Nothing;
        let loc = MemoryLocation::before_or_after(Operand::Value(llrm_mir::module::ValueId(0)));
        let mut batch = BatchAAResults::new(&mut nothing);
        assert!(!batch.points_to_constant_memory(&loc, false));
        assert!(!batch.is_no_alias(&loc, &loc));
        assert!(batch.cross_iteration_scope(true, |inner| inner.aaqi.may_be_cross_iteration));
        assert!(!batch.aaqi.may_be_cross_iteration);
        assert_eq!(MemoryEffects::none().overall(), ModRefInfo::NoModRef);
        assert_eq!(ModRefInfo::Ref.union(ModRefInfo::Mod), ModRefInfo::ModRef);
        assert_eq!(ModRefInfo::ModRef.intersect(ModRefInfo::Ref), ModRefInfo::Ref);
    }
}
