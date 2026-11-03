//! The interface of alias analysis, as LLVM's `AAResults` and `BatchAAResults`
//! (llvm/Analysis/AliasAnalysis.h at d1106deb71cc): the queries a client asks
//! (MemorySSA, MemDep, LICM, GVN, DSE), not an implementation.
//!
//! # How it works
//! Names are LLVM's in snake case; the primary queries are `alias`,
//! `get_mod_ref_info_mask`, `get_memory_effects`, `get_arg_mod_ref_info` and
//! the `get_mod_ref_info*` forms; every other query is a provided method over
//! those, as in LLVM.
//!
//! Nothing here keeps a fact about the whole function: a query is answered
//! from the uses and definitions it reaches, and what one query learns is kept
//! in the `AAQueryInfo` of its batch and dropped with it, never shared between
//! two versions of a body.
//!
//! # Example
//! A client asks whether a load and a store may touch the same bytes:
//!
//! ```text
//! let mut batch = AAQueryInfo::default();
//! let load = MemoryLocation::get(&unit, load_inst)?;
//! let store = MemoryLocation::get(&unit, store_inst)?;
//! if aa.alias(&load, &store, &mut batch, None) == AliasResult::NoAlias { /* independent */ }
//! ```
//!
//! # Provides
//! `LocationSize`, `AAInfo`, `MemoryLocation`, `AliasResult`, `ModRefInfo`,
//! `IRMemLocation`, `MemoryEffects`, `AAQueryInfo`, the `AAResults` trait and
//! `BatchAAResults`. Implemented by [`crate::basicaa`] (S1); no client reads
//! it yet.
//!
//! # See also
//! LLVM `AliasAnalysis.h`, `MemoryLocation.h`;
//! docs/architecture/basicaa-port.md.

use llrm_mir::module::{InstId, MetadataId, Operand};
use llrm_support::hash::HashMap;

/// How much of memory an access touches (`LocationSize`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LocationSize {
    /// Exactly `n` bytes from the pointer.
    Precise(u64),
    /// At most `n` bytes from the pointer, at an unknown place.
    UpperBound(u64),
    /// Any bytes before or after the pointer, within its object.
    BeforeOrAfterPointer,
    /// Any bytes after the pointer, within its object.
    AfterPointer,
    Unknown,
}

impl LocationSize {
    /// The bytes the location spans, where it says (`hasValue`): an upper bound
    /// or a precise size.
    pub fn value(self) -> Option<u64> {
        match self {
            Self::Precise(n) | Self::UpperBound(n) => Some(n),
            _ => None,
        }
    }

    pub fn is_precise(self) -> bool {
        matches!(self, Self::Precise(_))
    }

    /// Whether the access may start before its pointer.
    pub fn may_be_before_pointer(self) -> bool {
        matches!(self, Self::BeforeOrAfterPointer | Self::Unknown)
    }

    pub fn is_zero(self) -> bool {
        self == Self::Precise(0)
    }
}

/// Metadata an access carries that alias analysis reads (`AAMDNodes`).
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct AAInfo {
    pub tbaa: Option<MetadataId>,
    pub noalias_scope: Option<MetadataId>,
}

impl AAInfo {
    /// The tags an instruction carries.
    pub fn of(op: &llrm_mir::module::Instruction) -> Self {
        Self { tbaa: op.metadata.iter().find(|(kind, _)| kind == "tbaa").map(|(_, id)| *id), noalias_scope: None }
    }
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

    /// `MemoryLocation::get(Instruction)`: the place a load or a store
    /// accesses, its size the accessed type's store size and its tags the
    /// instruction's; none for a call, a fence or anything else
    /// (`getForArgument` is the call's).
    pub fn get(
        unit: &crate::memory::Unit,
        inst: InstId,
    ) -> Option<Self> {
        let function = unit.function;
        let op = function.instruction(inst);
        let (pointer, accessed) = match &op.opcode {
            llrm_mir::opcode::Opcode::Load { .. } => (op.operands[0], op.ty),
            llrm_mir::opcode::Opcode::Store { .. } => {
                (op.operands[1], function.operand_type(unit.context, op.operands[0])?)
            }
            _ => return None,
        };
        let bytes = unit.layout.store_size(&unit.context.types, accessed) as u64;
        Some(Self { ptr: pointer, size: LocationSize::Precise(bytes), aa_info: AAInfo::of(op) })
    }

    /// `MemoryLocation::getForArgument`: the place argument `arg` of `call`
    /// names, as far as the callee's attributes size it
    /// (`dereferenceable`), else unsized.
    pub fn get_for_argument(
        unit: &crate::memory::Unit,
        call: InstId,
        arg: u32,
    ) -> Option<Self> {
        let op = unit.function.instruction(call);
        let pointer = *op.operands.get(arg as usize)?;
        // `memset`, `memcpy` and `memmove` name the bytes they touch through
        // their destination and source (`getForDest`, `getForSource`).
        let named = llrm_mir::memory::callee(unit.context, unit.function, call)
            .and_then(|one| unit.globals.get(one.0 as usize)?.name.as_deref())
            .and_then(llrm_mir::intrinsics::Intrinsic::named);
        // `MemoryLocation.cpp#L200-L230`: the bytes an `invariant.start` or
        // `.end` covers are its constant size operand; the scope `.end` takes
        // first is a descriptor never dereferenced.
        match named {
            Some(llrm_mir::intrinsics::Intrinsic::InvariantStart) if arg == 1 => {
                let size = llrm_mir::intrinsics::length(unit.context, *op.operands.first()?)?;
                return Some(Self::new(pointer, LocationSize::Precise(size)));
            }
            Some(llrm_mir::intrinsics::Intrinsic::InvariantEnd) if arg == 0 => {
                return Some(Self::new(pointer, LocationSize::Precise(0)));
            }
            Some(llrm_mir::intrinsics::Intrinsic::InvariantEnd) if arg == 2 => {
                let size = llrm_mir::intrinsics::length(unit.context, *op.operands.get(1)?)?;
                return Some(Self::new(pointer, LocationSize::Precise(size)));
            }
            _ => {}
        }
        let touches = match named {
            Some(llrm_mir::intrinsics::Intrinsic::MemSet) => arg == 0,
            Some(llrm_mir::intrinsics::Intrinsic::MemCpy | llrm_mir::intrinsics::Intrinsic::MemMove) => arg < 2,
            _ => false,
        };
        if touches && let Some(length) = llrm_mir::intrinsics::length(unit.context, *op.operands.get(2)?) {
            return Some(Self::new(pointer, LocationSize::Precise(length)));
        }
        Some(Self::before_or_after(pointer))
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

impl AliasResult {
    /// The result for the pair the other way round (`AliasResult::swap`): a
    /// partial alias's offset changes sign.
    pub fn swap(self) -> Self {
        match self {
            Self::PartialAlias { offset } => Self::PartialAlias { offset: offset.map(|one| -one) },
            other => other,
        }
    }
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

    /// `Attrs.getMemoryEffects()`: what `memory(...)` and the legacy
    /// `readnone`/`readonly`/`writeonly` allow at each location, the
    /// attributes' meet. A location `memory(...)` does not name takes its
    /// default, `none` when the default is not stated.
    pub fn of_attrs(attrs: &[llrm_mir::opcode::Attribute]) -> Self {
        use llrm_mir::opcode::Attribute;
        let access = |word: &str| match word {
            "readwrite" => ModRefInfo::ModRef,
            "read" => ModRefInfo::Ref,
            "write" => ModRefInfo::Mod,
            _ => ModRefInfo::NoModRef,
        };
        let mut result = Self::unknown();
        for attr in attrs {
            let stated = match attr {
                Attribute::Memory(locations) => {
                    let default = locations
                        .iter()
                        .find(|(name, _)| name.is_none())
                        .map_or(ModRefInfo::NoModRef, |(_, word)| access(word));
                    let named = |name: &str| {
                        locations
                            .iter()
                            .find(|(one, _)| one.as_deref() == Some(name))
                            .map_or(default, |(_, word)| access(word))
                    };
                    Self::none()
                        .with(IRMemLocation::ArgMem, named("argmem"))
                        .with(IRMemLocation::InaccessibleMem, named("inaccessiblemem"))
                        .with(IRMemLocation::ErrnoMem, named("errnomem"))
                        .with(IRMemLocation::Other, default)
                }
                Attribute::Flag(_) => {
                    let through = llrm_mir::memory::through(std::slice::from_ref(attr));
                    let info = match (through.reads, through.writes) {
                        (true, true) => ModRefInfo::ModRef,
                        (true, false) => ModRefInfo::Ref,
                        (false, true) => ModRefInfo::Mod,
                        (false, false) => ModRefInfo::NoModRef,
                    };
                    Self { effects: [info; 4] }
                }
                _ => continue,
            };
            result = result.intersect(stated);
        }
        result
    }

    /// `MemoryEffects::operator|`.
    pub fn union(
        self,
        other: Self,
    ) -> Self {
        Self { effects: std::array::from_fn(|at| self.effects[at].union(other.effects[at])) }
    }

    /// `MemoryEffects::argMemOnly(MR)`.
    pub fn arg_mem_only(info: ModRefInfo) -> Self {
        Self::none().with(IRMemLocation::ArgMem, info)
    }

    /// `MemoryEffects::getWithoutLoc`.
    pub fn without_loc(
        self,
        at: IRMemLocation,
    ) -> Self {
        self.with(at, ModRefInfo::NoModRef)
    }

    /// The same `info` at every location (`MemoryEffects(MR)`).
    pub fn everywhere(info: ModRefInfo) -> Self {
        Self { effects: [info; 4] }
    }

    /// The `memory(...)` attribute that states `self`, as LLVM prints it: the
    /// default is `Other`'s, and a location is named when it differs.
    pub fn attribute(self) -> llrm_mir::opcode::Attribute {
        let word = |info: ModRefInfo| {
            match info {
                ModRefInfo::NoModRef => "none",
                ModRefInfo::Ref => "read",
                ModRefInfo::Mod => "write",
                ModRefInfo::ModRef => "readwrite",
            }
            .to_owned()
        };
        let other = self.mod_ref(IRMemLocation::Other);
        let named = [
            (IRMemLocation::ArgMem, "argmem"),
            (IRMemLocation::InaccessibleMem, "inaccessiblemem"),
            (IRMemLocation::ErrnoMem, "errnomem"),
        ]
        .into_iter()
        .filter(|&(at, _)| self.mod_ref(at) != other)
        .map(|(at, name)| (Some(name.to_owned()), word(self.mod_ref(at))))
        .collect::<Vec<_>>();
        let mut locations = Vec::new();
        if other != ModRefInfo::NoModRef || named.is_empty() {
            locations.push((None, word(other)));
        }
        locations.extend(named);
        llrm_mir::opcode::Attribute::Memory(locations)
    }

    /// `MemoryEffects::operator&`.
    pub fn intersect(
        self,
        other: Self,
    ) -> Self {
        Self { effects: std::array::from_fn(|at| self.effects[at].intersect(other.effects[at])) }
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

/// What the queries of one batch keep (`AAQueryInfo`): the locations asked
/// about, numbered, and the answer for each pair by those numbers (a pair is
/// asked in one order, `swap` gives the other), and the two switches a walk
/// sets.
#[derive(Debug)]
pub struct AAQueryInfo {
    /// The query may be asked about different iterations of a loop
    /// (`MayBeCrossIteration`).
    pub may_be_cross_iteration: bool,
    /// Whether the dominator tree may be used (`UseDominatorTree`).
    pub use_dominator_tree: bool,
    locations: HashMap<(MemoryLocation, bool), u32>,
    answers: HashMap<u64, Answer>,
    /// `NumAssumptionUses`: the uses of an assumption still in force.
    assumption_uses: i32,
    /// `AssumptionBasedResults`: the pairs whose answer rests on an assumption
    /// of a query not yet decided.
    assumption_based: Vec<u64>,
    /// `AAQI.CA`: how objects may be captured before an instruction. A batch
    /// owns it, so a batch drops what it remembers.
    pub capture: Box<dyn crate::captureanalysis::CaptureAnalysis>,
    /// What the implementation classified of an object, once for the batch.
    pub classes: HashMap<llrm_mir::module::Operand, u8>,
    pub(crate) decomposed: HashMap<llrm_mir::module::Operand, std::rc::Rc<crate::basicaa::Decomposed>>,
}

impl AAQueryInfo {
    /// A batch whose capture questions go to `capture` (`AAQueryInfo(AAR,
    /// CA)`).
    pub fn with_capture(capture: Box<dyn crate::captureanalysis::CaptureAnalysis>) -> Self {
        Self { capture, ..Self::default() }
    }
}

/// `AAQueryInfo::CacheEntry`: an answer, and how many uses of it were made as
/// an assumption while it was still being worked out (`isDefinitive`: -1).
#[derive(Clone, Copy, Debug)]
struct Answer {
    result: AliasResult,
    uses: i32,
}

/// What `enter` found.
pub enum Entered {
    /// The pair is answered (a definitive answer, or the assumption).
    Known(AliasResult),
    /// A new pair, to be worked out, with the state `exit` needs.
    Fresh(Mark),
}

/// The state of the assumptions on entering a pair.
#[derive(Clone, Copy, Debug)]
pub struct Mark {
    uses: i32,
    based: usize,
    key: u64,
}

impl Default for AAQueryInfo {
    fn default() -> Self {
        Self {
            may_be_cross_iteration: false,
            use_dominator_tree: true,
            locations: HashMap::default(),
            answers: HashMap::default(),
            assumption_uses: 0,
            assumption_based: Vec::new(),
            capture: Box::<crate::captureanalysis::SimpleCaptureAnalysis>::default(),
            classes: HashMap::default(),
            decomposed: HashMap::default(),
        }
    }
}

impl AAQueryInfo {
    fn number(
        &mut self,
        location: &MemoryLocation,
    ) -> u32 {
        let next = self.locations.len() as u32;
        // MayBeCrossIteration is part of the pair's key: some answers of the
        // other mode become MayAlias in this one.
        *self.locations.entry((*location, self.may_be_cross_iteration)).or_insert(next)
    }

    /// The remembered answer for the pair, in the order asked.
    pub fn cached(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
    ) -> Option<AliasResult> {
        let (one, other) = (self.number(a), self.number(b));
        if let Some(found) = self.answers.get(&(u64::from(one) << 32 | u64::from(other))) {
            return Some(found.result);
        }
        self.answers.get(&(u64::from(other) << 32 | u64::from(one))).map(|found| found.result.swap())
    }

    pub fn remember(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        answer: AliasResult,
    ) {
        let (one, other) = (self.number(a), self.number(b));
        self.answers.insert(u64::from(one) << 32 | u64::from(other), Answer { result: answer, uses: -1 });
    }

    /// The first half of `BasicAAResult::aliasCheck`'s cache use: a pair asked
    /// again while it is being worked out is assumed `NoAlias`, and the use is
    /// counted.
    pub fn enter(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
    ) -> Entered {
        let (one, other) = (self.number(a), self.number(b));
        let (forward, backward) = (u64::from(one) << 32 | u64::from(other), u64::from(other) << 32 | u64::from(one));
        for (key, swapped) in [(forward, false), (backward, true)] {
            if let Some(entry) = self.answers.get_mut(&key) {
                if entry.uses >= 0 {
                    entry.uses += 1;
                    self.assumption_uses += 1;
                }
                return Entered::Known(if swapped { entry.result.swap() } else { entry.result });
            }
        }
        self.answers.insert(forward, Answer { result: AliasResult::NoAlias, uses: 0 });
        Entered::Fresh(Mark { uses: self.assumption_uses, based: self.assumption_based.len(), key: forward })
    }

    /// The second half: `result` of the pair `enter` made fresh. An assumption
    /// of `NoAlias` that was used and is not borne out makes the answer
    /// `MayAlias` and drops every answer that rested on it.
    pub fn exit(
        &mut self,
        mark: Mark,
        result: AliasResult,
    ) -> AliasResult {
        let entry = self.answers.get_mut(&mark.key).expect("the pair is in the cache after its entry");
        let disproven = entry.uses > 0 && result != AliasResult::NoAlias;
        let result = if disproven { AliasResult::MayAlias } else { result };
        // Definitive now, as a root query.
        self.assumption_uses -= entry.uses;
        entry.result = result;
        entry.uses = -1;
        if disproven {
            while self.assumption_based.len() > mark.based {
                if let Some(key) = self.assumption_based.pop() {
                    self.answers.remove(&key);
                }
            }
        }
        // Still resting on an assumption higher up: remembered for purging.
        if self.assumption_uses > mark.uses {
            self.assumption_based.push(mark.key);
        }
        result
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

    /// `get_mod_ref_info` where `others`, the other providers of an aggregate,
    /// also answer each `alias` a call's argument is asked against `loc`
    /// (`AAQI.AAR.alias`, BasicAliasAnalysis.cpp L1030): without them a
    /// provider answers from its own `alias` alone.
    fn get_mod_ref_info_with(
        &mut self,
        inst: InstId,
        loc: Option<&MemoryLocation>,
        aaqi: &mut AAQueryInfo,
        others: &mut dyn FnMut(&MemoryLocation, &MemoryLocation, &mut AAQueryInfo, Option<InstId>) -> AliasResult,
    ) -> ModRefInfo {
        let _ = others;
        self.get_mod_ref_info(inst, loc, aaqi)
    }

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

    fn only_writes_memory(
        &mut self,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> bool {
        let effects = self.get_memory_effects(call, aaqi);
        !effects.overall().is_ref_set()
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

    pub fn only_writes_memory(
        &mut self,
        call: InstId,
    ) -> bool {
        self.aa.only_writes_memory(call, &mut self.aaqi)
    }

    pub fn does_not_access_memory(
        &mut self,
        call: InstId,
    ) -> bool {
        self.aa.does_not_access_memory(call, &mut self.aaqi)
    }

    pub fn only_reads_memory(
        &mut self,
        call: InstId,
    ) -> bool {
        self.aa.only_reads_memory(call, &mut self.aaqi)
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
    use crate::memory::Unit;
    use crate::testing::{DOS, function, layout, parsed};

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

    /// A pair asked one way and again the other is one question: the answer is
    /// kept by the interned ids of the two locations, and a partial alias's
    /// offset changes sign when read the other way (LLVM's
    /// `AliasResult::swap`).
    #[test]
    fn a_remembered_pair_is_read_back_the_other_way_with_the_offset_swapped() {
        let (a, b) = (
            MemoryLocation::new(Operand::Value(llrm_mir::module::ValueId(1)), LocationSize::Precise(4)),
            MemoryLocation::new(Operand::Value(llrm_mir::module::ValueId(2)), LocationSize::Precise(4)),
        );
        let mut batch = AAQueryInfo::default();
        assert_eq!(batch.cached(&a, &b), None);
        batch.remember(&a, &b, AliasResult::PartialAlias { offset: Some(8) });
        assert_eq!(batch.cached(&a, &b), Some(AliasResult::PartialAlias { offset: Some(8) }));
        assert_eq!(batch.cached(&b, &a), Some(AliasResult::PartialAlias { offset: Some(-8) }));
        assert_eq!(AliasResult::NoAlias.swap(), AliasResult::NoAlias);
    }

    /// The place a load or a store accesses is its pointer and the type's store
    /// size; a call has none.
    #[test]
    fn the_location_of_an_instruction_is_its_pointer_and_the_accessed_size() {
        let module = parsed(&format!(
            "{DOS}declare void @g()\ndefine void @f(ptr %p) {{\nentry:\n  %v = load i16, ptr %p\n  store i32 7, ptr %p\n  call void @g()\n  ret void\n}}\n"
        ));
        let layout = layout(&module);
        let f = function(&module, "f");
        let unit = Unit::of(&module, &layout, f);
        let sizes: Vec<Option<LocationSize>> = f
            .walk()
            .map(|(_, inst)| inst)
            .filter(|&inst| !matches!(f.instruction(inst).opcode, llrm_mir::opcode::Opcode::Ret))
            .map(|inst| MemoryLocation::get(&unit, inst).map(|one| one.size))
            .collect();
        assert_eq!(sizes, vec![Some(LocationSize::Precise(2)), Some(LocationSize::Precise(4)), None]);
    }
}
