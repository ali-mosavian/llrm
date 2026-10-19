//! `MemoryDependenceAnalysis`: for a load, a store or a call, the nearest
//! instruction it depends on in its block (a def or a clobber), or that there
//! is none there and the question moves to the predecessors.
//!
//! A translation of LLVM's
//! [`MemoryDependenceAnalysis.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp)
//! and `.h` at d1106deb71cc, over the interface of [`crate::aa`]. Each function
//! names the C++ function it translates and keeps its order of checks; caps
//! stand at their `cl::opt` defaults.
//!
//! # How it works
//! `get_dependency` scans a block backwards from the query, asking the alias
//! analysis about each instruction, and caches the answer with a reverse map
//! (`reverse_local_deps`) so `remove_instruction` can dirty exactly the answers
//! that named the removed instruction. A dirty answer keeps the instruction
//! after the removed one, so a rescan starts there.
//! `get_non_local_call_dependency` does the same per predecessor block for a
//! call.
//!
//! # Example
//! ```text
//! b0: store i16 1, ptr %p     ; <- Def of the load below (must alias)
//!     call void @g()          ;    writes memory it cannot name: Clobber of anything
//!     %x = load i16, ptr %p   ;    get_dependency(%x) = Clobber(call)
//! ```
//!
//! # Provides
//! [`MemDepResult`], [`MemoryDependenceResults`] (`get_dependency`,
//! `get_non_local_call_dependency`, `get_non_local_pointer_dependency`,
//! `invalidate_cached_pointer_info`, `get_pointer_dependency_from`,
//! `get_simple_pointer_dependency_from`, `remove_instruction`,
//! `invalidate_cached_predecessors`, `verify_removed`), [`get_location`] and
//! [`is_store_preserving_memory_location`] (`Loads.cpp`, shared with GVN).
//!
//! # See also
//! docs/architecture/gvn-port.md (stage 2).
//!
//! DEVIATE, permanent (MIR lacks the construct):
//! 1. Atomics, fences, `va_arg`, masked loads and stores,
//!    `invariant.start`/`end`: MIR has none, so the ordering and fence arms of
//!    `GetLocation` and of the scan are absent. A volatile access has no
//!    location (it is `isUnordered` false).
//! 2. `getInvariantGroupPointerDependency` (and with it `NonLocalDefsCache`,
//!    only it fills): MIR has no `invariant.group`. A query on an instruction
//!    that carries one panics, `test_invariant_group_metadata_is_refused`.
//! 3. `getFreedOperand`: there is no `TargetLibraryInfo`, so a call to `free`
//!    is a call with no location (it clobbers by its attributes).
//!
//! DEVIATE, stated facts:
//! 4. One `BatchAAResults` per scan, not one per `AAResults` call: it only
//!    memoises `alias` for one body, which does not change during a scan.
//! 5. `EarliestEscapeAnalysis` belongs to the alias analysis (the batch takes
//!    no `EEA`), so `remove_instruction` forwards nothing to it; an alias
//!    analysis is made for one version of a body (`aa.rs`).
//! 6. `predecessors` names each predecessor once; C++ names a block once per
//!    edge. The repeats only revisit a visited block.
//! 7. A call is identical when its `tail` is a tail call in both, not the same
//!    kind: `isIdenticalToWhenDefined` compares `isTailCall`.
//!
//! OWED: the non-local pointer queries (`getNonLocalPointerDependency` and its
//! helpers, the pointer caches in `removeInstruction`,
//! `invalidateCachedPointerInfo`): phi translation interns constants, which
//! needs `&mut Context` while the alias analysis holds `&Context` (#2500).

use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::domtree::DomTree;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Opcode, Tail};
use llrm_mir::valuetracking::{self, Query};
use llrm_support::hash::{HashMap, HashSet};

use crate::aa::{AAInfo, AAResults, AliasResult, BatchAAResults, LocationSize, MemoryLocation, ModRefInfo};
use crate::effects;
use crate::memory::Unit;
use crate::mssa::phitrans::{Env as PhiEnv, PhiTransAddr, SelectAddrs};

/// [`BlockScanLimit`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L65),
/// `memdep-block-scan-limit`.
pub const BLOCK_SCAN_LIMIT: u32 = 100;

/// [`MemDepResult`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemoryDependenceAnalysis.h#L52):
/// one of Clobber, Def, or Other with its detail; `Invalid` is the dirty state,
/// holding an instruction to rescan from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemDepResult {
    /// Clients never see this: the answer was invalidated; the instruction, if
    /// any, is where a rescan can start.
    Invalid(Option<InstId>),
    Clobber(InstId),
    Def(InstId),
    /// No dependence in the block; ask the predecessors.
    NonLocal,
    /// No dependence in the function.
    NonFuncLocal,
    /// The address is a select whose two sides each reach a non-clobbered
    /// value.
    Select,
    Unknown,
}

/// `MemDepResult()` is dirty, with no instruction.
impl Default for MemDepResult {
    fn default() -> Self {
        Self::Invalid(None)
    }
}

impl MemDepResult {
    pub fn is_clobber(self) -> bool {
        matches!(self, Self::Clobber(_))
    }

    pub fn is_def(self) -> bool {
        matches!(self, Self::Def(_))
    }

    /// `isLocal`: Clobber or Def.
    pub fn is_local(self) -> bool {
        self.is_clobber() || self.is_def()
    }

    pub fn is_non_local(self) -> bool {
        self == Self::NonLocal
    }

    pub fn is_non_func_local(self) -> bool {
        self == Self::NonFuncLocal
    }

    pub fn is_select(self) -> bool {
        self == Self::Select
    }

    pub fn is_unknown(self) -> bool {
        self == Self::Unknown
    }

    pub fn is_dirty(self) -> bool {
        matches!(self, Self::Invalid(_))
    }

    /// `getInst`: the instruction depended on; for a dirty answer, where to
    /// rescan from.
    pub fn inst(self) -> Option<InstId> {
        match self {
            Self::Invalid(inst) => inst,
            Self::Clobber(inst) | Self::Def(inst) => Some(inst),
            _ => None,
        }
    }
}

/// [`NonLocalDepEntry`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemoryDependenceAnalysis.h#L196):
/// the answer for one block; the cache is sorted by block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonLocalDepEntry {
    pub bb: BlockId,
    pub result: MemDepResult,
}

/// `NonLocalDepInfo`.
pub type NonLocalDepInfo = Vec<NonLocalDepEntry>;

/// What a scan reads: the function and its module's types and attributes, and
/// its dominator tree.
#[derive(Clone, Copy)]
pub struct Env<'a> {
    pub unit: &'a Unit<'a>,
    pub dt: &'a DomTree,
}

/// `SelectAddr`: the address of a non-local result, or the select condition and
/// the addresses on its two sides.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectAddr {
    Addr(Option<Operand>),
    Select { cond: Operand, addrs: (Operand, Operand) },
}

/// [`NonLocalDepResult`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemoryDependenceAnalysis.h#L230):
/// the dependence in one block and the address that was live in it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonLocalDepResult {
    pub bb: BlockId,
    pub result: MemDepResult,
    pub address: SelectAddr,
}

/// `ValueIsLoadPair`: a pointer and whether the dependence is of a read.
type ValueIsLoadPair = (Operand, bool);

/// `NonLocalPointerInfo`: the answers for one pointer, by block.
#[derive(Clone, Debug)]
struct NonLocalPointerInfo {
    /// The block and skip-first-block flag of the full query these answers are
    /// of; none when they are not.
    pair: (Option<BlockId>, bool),
    non_local_deps: NonLocalDepInfo,
    size: LocationSize,
    aa_tags: AAInfo,
}

impl NonLocalPointerInfo {
    fn new(
        size: LocationSize,
        aa_tags: AAInfo,
    ) -> Self {
        Self { pair: (None, false), non_local_deps: Vec::new(), size, aa_tags }
    }
}

/// [`NumResultsLimit`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L81).
const NUM_RESULTS_LIMIT: usize = 100;
/// `memdep-block-number-limit`.
const BLOCK_NUMBER_LIMIT: usize = 200;
/// `memdep-cache-global-limit`.
const CACHE_GLOBAL_LIMIT: usize = 10000;

/// [`RemoveFromReverseMap`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L96).
fn remove_from_reverse_map(
    reverse: &mut IdMap<InstId, IdSet<InstId>>,
    inst: InstId,
    value: InstId,
) {
    let set = reverse.get_mut(&inst).expect("Reverse map out of sync?");
    assert!(set.remove(&value), "Invalid reverse map!");
    if set.is_empty() {
        reverse.remove(&inst);
    }
}

/// `RemoveFromReverseMap` for the pointer pairs.
fn remove_from_reverse_pairs(
    reverse: &mut IdMap<InstId, HashSet<ValueIsLoadPair>>,
    inst: InstId,
    value: ValueIsLoadPair,
) {
    let set = reverse.get_mut(&inst).expect("Reverse map out of sync?");
    assert!(set.remove(&value), "Invalid reverse map!");
    if set.is_empty() {
        reverse.remove(&inst);
    }
}

/// [`GetLocation`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L113):
/// the location `inst` accesses, if it names one, and what it does to memory.
pub fn get_location(
    unit: &Unit,
    inst: InstId,
) -> (Option<MemoryLocation>, ModRefInfo) {
    let function = unit.function;
    match &function.instruction(inst).opcode {
        // A volatile access is not `isUnordered`: no location.
        Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. } => (None, ModRefInfo::ModRef),
        Opcode::Load { .. } => (MemoryLocation::get(unit, inst), ModRefInfo::Ref),
        Opcode::Store { .. } => (MemoryLocation::get(unit, inst), ModRefInfo::Mod),
        opcode => {
            if matches!(opcode, Opcode::Call(_))
                && matches!(
                    unit.intrinsic(inst),
                    Some(Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd)
                )
            {
                // They don't modify the memory, but Mod makes them
                // conservative.
                return (MemoryLocation::get_for_argument(unit, inst, 0), ModRefInfo::Mod);
            }
            // The coarse-grained thing that always works.
            let calls = matches!(opcode, Opcode::Call(_) | Opcode::Invoke(_));
            let effects = if calls {
                effects::call(unit.context, unit.globals, function, inst)
            } else {
                llrm_mir::memory::Effects::NONE
            };
            if effects.writes {
                (None, ModRefInfo::ModRef)
            } else if effects.reads {
                (None, ModRefInfo::Ref)
            } else {
                (None, ModRefInfo::NoModRef)
            }
        }
    }
}

/// `Instruction::isIdenticalToWhenDefined` of two calls (DEVIATE 7).
fn identical_calls(
    function: &Function,
    a: InstId,
    b: InstId,
) -> bool {
    let (a, b) = (function.instruction(a), function.instruction(b));
    let (Opcode::Call(x), Opcode::Call(y)) = (&a.opcode, &b.opcode) else { return false };
    let tail = |tail: Tail| matches!(tail, Tail::Tail | Tail::MustTail);
    a.ty == b.ty
        && a.operands == b.operands
        && x.function_type == y.function_type
        && x.calling_convention == y.calling_convention
        && x.return_attrs == y.return_attrs
        && x.argument_attrs == y.argument_attrs
        && x.attrs == y.attrs
        && tail(x.tail) == tail(y.tail)
}

/// [`isStorePreservingMemoryLocation`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/Loads.cpp#L843)
/// (Loads.cpp): `store` writes back, unchanged, what `mem_loc` holds: it stores
/// a load of the same bytes, same block, and nothing between modifies them.
pub fn is_store_preserving_memory_location<A: AAResults>(
    unit: &Unit,
    store: InstId,
    mem_loc: &MemoryLocation,
    mem_loc_align: u64,
    batch: &mut BatchAAResults<A>,
    scan_limit: u32,
) -> bool {
    let function = unit.function;
    // No partial overlap is possible, and the stored value is the current
    // content of `mem_loc`.
    let Some(size) = mem_loc.size.value() else { return false };
    let Some(store_loc) = MemoryLocation::get(unit, store) else { return false };
    if store_loc.size != mem_loc.size {
        return false;
    }
    let Opcode::Store { align, .. } = function.instruction(store).opcode else { return false };
    let store_align = align.unwrap_or_else(|| {
        let ty = function.operand_type(unit.context, function.instruction(store).operands[0]).expect("a stored value");
        unit.layout.align(&unit.context.types, ty)
    });
    if mem_loc_align.min(store_align) < size {
        return false;
    }
    let Operand::Value(stored) = function.instruction(store).operands[0] else { return false };
    let ValueDef::Instruction(load) = function.value(stored).def else { return false };
    if !matches!(function.instruction(load).opcode, Opcode::Load { .. })
        || function.parent(load) != function.parent(store)
    {
        return false;
    }
    let Some(load_loc) = MemoryLocation::get(unit, load) else { return false };
    if batch.alias(&load_loc, mem_loc) != AliasResult::MustAlias {
        return false;
    }
    // No memory operation in between may modify `mem_loc`.
    let mut visited = 0;
    let mut at = Some(load);
    while let Some(inst) = at.filter(|&inst| inst != store) {
        visited += 1;
        if visited > scan_limit || batch.get_mod_ref_info(inst, Some(mem_loc)).is_mod_set() {
            return false;
        }
        at = function.next_in_block(inst);
    }
    true
}

/// The cached answers and their reverse maps.
#[derive(Debug)]
pub struct MemoryDependenceResults {
    local_deps: IdMap<InstId, MemDepResult>,
    non_local_deps_map: IdMap<InstId, (NonLocalDepInfo, bool)>,
    reverse_local_deps: IdMap<InstId, IdSet<InstId>>,
    reverse_non_local_deps: IdMap<InstId, IdSet<InstId>>,
    non_local_pointer_deps: HashMap<ValueIsLoadPair, NonLocalPointerInfo>,
    reverse_non_local_ptr_deps: IdMap<InstId, HashSet<ValueIsLoadPair>>,
    /// `NonLocalPointerDepVisited`: for each block, the pointer it was reached
    /// with in the query of this epoch.
    visited: Vec<(Option<Operand>, u32)>,
    epoch: u32,
    pred_cache: IdMap<BlockId, Vec<BlockId>>,
    clobber_offsets: IdMap<InstId, i32>,
    default_block_scan_limit: u32,
}

impl Default for MemoryDependenceResults {
    fn default() -> Self {
        Self::new(BLOCK_SCAN_LIMIT)
    }
}

impl MemoryDependenceResults {
    pub fn new(default_block_scan_limit: u32) -> Self {
        Self {
            local_deps: IdMap::new(),
            non_local_deps_map: IdMap::new(),
            reverse_local_deps: IdMap::new(),
            reverse_non_local_deps: IdMap::new(),
            non_local_pointer_deps: HashMap::default(),
            reverse_non_local_ptr_deps: IdMap::new(),
            visited: Vec::new(),
            epoch: 0,
            pred_cache: IdMap::new(),
            clobber_offsets: IdMap::new(),
            default_block_scan_limit,
        }
    }

    /// `getDefaultBlockScanLimit`.
    pub fn default_block_scan_limit(&self) -> u32 {
        self.default_block_scan_limit
    }

    /// `getClobberOffset`: the offset a partially aliasing load clobbered at.
    pub fn clobber_offset(
        &self,
        load: InstId,
    ) -> Option<i32> {
        self.clobber_offsets.get(&load).copied()
    }

    /// `PredIteratorCache::get`.
    fn preds(
        &mut self,
        function: &Function,
        bb: BlockId,
    ) -> Vec<BlockId> {
        self.pred_cache.get_or_insert_with(bb, || function.predecessors(bb).to_vec()).clone()
    }

    /// `invalidateCachedPredecessors`
    /// ([:1517](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1517)).
    pub fn invalidate_cached_predecessors(&mut self) {
        self.pred_cache.clear();
    }

    /// The instruction before `scan` in `bb`: `scan` is where the walk stands
    /// (`ScanIt`), none for the block's end.
    fn before(
        function: &Function,
        bb: BlockId,
        scan: Option<InstId>,
    ) -> Option<InstId> {
        match scan {
            Some(inst) => function.prev_in_block(inst),
            None => function.block(bb).last(),
        }
    }

    /// `getCallDependencyFrom`
    /// ([:192](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L192)):
    /// the dependence of `call` found scanning `bb` backwards from `scan`.
    fn get_call_dependency_from<A: AAResults>(
        &self,
        env: Env,
        aa: &mut A,
        call: InstId,
        is_read_only_call: bool,
        mut scan: Option<InstId>,
        bb: BlockId,
    ) -> MemDepResult {
        let (unit, function) = (env.unit, env.unit.function);
        let mut limit = self.default_block_scan_limit;
        let is_invariant_load = function.instruction(call).metadata.iter().any(|(kind, _)| kind == "invariant.load");
        let mut batch = BatchAAResults::new(aa);
        // Walk backwards through the block, looking for dependencies.
        while let Some(inst) = Self::before(function, bb, scan) {
            scan = Some(inst);
            // A cap on the scan so extreme tests are not quadratic.
            limit = limit.wrapping_sub(1);
            if limit == 0 {
                return MemDepResult::Unknown;
            }
            // If this inst is a memory op, get the pointer it accessed.
            let (loc, mr) = get_location(unit, inst);
            if let Some(loc) = loc {
                // A simple instruction.
                if !batch.get_mod_ref_info(call, Some(&loc)).is_no_mod_ref() {
                    if is_invariant_load {
                        continue;
                    }
                    return MemDepResult::Clobber(inst);
                }
                continue;
            }
            if matches!(function.instruction(inst).opcode, Opcode::Call(_)) {
                let identical_read_only_call =
                    is_read_only_call && !mr.is_mod_set() && identical_calls(function, call, inst);
                // An identical earlier invariant load-like call is an available
                // value even if AA sees both calls as reading the same memory.
                if is_invariant_load && identical_read_only_call {
                    return MemDepResult::Def(inst);
                }
                // If these two calls do not interfere, look past it.
                if batch.get_mod_ref_info_call(call, inst).is_no_mod_ref() {
                    // If the two calls are the same, `inst` is a Def, so `call`
                    // can be found redundant and eliminated.
                    if identical_read_only_call {
                        return MemDepResult::Def(inst);
                    }
                    // Otherwise the calls don't interact (e.g. `inst` is
                    // readnone): keep scanning.
                    continue;
                } else if is_invariant_load {
                    continue;
                }
                return MemDepResult::Clobber(inst);
            }
            // No pointer for the instruction and it touches memory: assume it
            // is a dependency.
            if mr != ModRefInfo::NoModRef {
                return MemDepResult::Clobber(inst);
            }
        }
        // No dependence found. In the entry block of the function it is
        // unknown, otherwise non-local.
        if Some(bb) != function.entry() { MemDepResult::NonLocal } else { MemDepResult::NonFuncLocal }
    }

    /// `getPointerDependencyFrom` with a batch
    /// ([:260](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L260)):
    /// without `invariant.group` (DEVIATE 2) it is the simple dependence.
    pub fn get_pointer_dependency_from_with<A: AAResults>(
        &mut self,
        env: Env,
        batch: &mut BatchAAResults<A>,
        mem_loc: &MemoryLocation,
        is_load: bool,
        scan: Option<InstId>,
        bb: BlockId,
        query: Option<InstId>,
        limit: Option<&mut u32>,
    ) -> MemDepResult {
        assert!(
            query.is_none_or(|inst| {
                !env.unit.function.instruction(inst).metadata.iter().any(|(kind, _)| kind == "invariant.group")
            }),
            "invariant.group is not modelled (DEVIATE 2)"
        );
        self.get_simple_pointer_dependency_from(env, batch, mem_loc, is_load, scan, bb, query, limit)
    }

    /// `getPointerDependencyFrom`
    /// ([:288](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L288)):
    /// the dependence of `mem_loc` scanning `bb` backwards from `scan`.
    pub fn get_pointer_dependency_from<A: AAResults>(
        &mut self,
        env: Env,
        aa: &mut A,
        mem_loc: &MemoryLocation,
        is_load: bool,
        scan: Option<InstId>,
        bb: BlockId,
        query: Option<InstId>,
        limit: Option<&mut u32>,
    ) -> MemDepResult {
        let mut batch = BatchAAResults::new(aa);
        self.get_pointer_dependency_from_with(env, &mut batch, mem_loc, is_load, scan, bb, query, limit)
    }

    /// `getSimplePointerDependencyFrom`
    /// ([:354](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L354)).
    pub fn get_simple_pointer_dependency_from<A: AAResults>(
        &mut self,
        env: Env,
        batch: &mut BatchAAResults<A>,
        mem_loc: &MemoryLocation,
        is_load: bool,
        mut scan: Option<InstId>,
        bb: BlockId,
        query: Option<InstId>,
        limit: Option<&mut u32>,
    ) -> MemDepResult {
        let (unit, function) = (env.unit, env.unit.function);
        let mut default_limit = self.default_block_scan_limit;
        let limit = limit.unwrap_or(&mut default_limit);
        let mut is_invariant_load = false;
        let mut mem_loc_align = {
            let q = Query {
                context: unit.context,
                globals: unit.globals,
                metadata: unit.metadata,
                function,
                layout: unit.layout,
                dt: None,
                assumptions: None,
                dom_conditions: None,
                cond_context: None,
                cxt: None,
            };
            valuetracking::known::pointer_alignment(&q, mem_loc.ptr)
        };
        // An invariant load "knows" it doesn't alias any write; must-alias
        // results still count, since defs are useful for value forwarding.
        if is_load && let Some(query) = query {
            let instruction = function.instruction(query);
            is_invariant_load = instruction.metadata.iter().any(|(kind, _)| kind == "invariant.load");
            if let Opcode::Load { align, .. } = instruction.opcode {
                mem_loc_align = align.unwrap_or_else(|| unit.layout.align(&unit.context.types, instruction.ty));
            }
        }
        // Walk backwards through the basic block, looking for dependencies.
        while let Some(inst) = Self::before(function, bb, scan) {
            scan = Some(inst);
            // A cap on the scan so extreme tests are not quadratic.
            *limit = limit.wrapping_sub(1);
            if *limit == 0 {
                return MemDepResult::Unknown;
            }
            // At a lifetime begin or end marker the query ends: the value is
            // undefined.
            if matches!(function.instruction(inst).opcode, Opcode::Call(_))
                && unit.intrinsic(inst) == Some(Intrinsic::LifetimeStart)
            {
                let arg_loc = MemoryLocation::get_for_argument(unit, inst, 0).expect("lifetime names its object");
                match batch.alias(&arg_loc, mem_loc) {
                    AliasResult::MustAlias => return MemDepResult::Def(inst),
                    AliasResult::NoAlias => continue,
                    // A partial overlap must act as a barrier.
                    _ => return MemDepResult::Clobber(inst),
                }
            }
            // Values depend on loads if the pointers are must aliased: a load
            // depends on another must aliased load from the same value.
            if matches!(function.instruction(inst).opcode, Opcode::Load { .. }) {
                // A volatile access need not clobber non-aliasing locations.
                if matches!(
                    function.instruction(inst).opcode,
                    Opcode::Load { volatile: true, .. }
                ) {
                    match query {
                        // The original query may be volatile.
                        None => return MemDepResult::Clobber(inst),
                        Some(query) if is_volatile(function, query) => return MemDepResult::Clobber(inst),
                        // Otherwise volatile implies no ordering.
                        Some(_) => {}
                    }
                }
                let load_loc = MemoryLocation::get(unit, inst).expect("a load");
                // A pointer was found: could it be the same as ours?
                let result = batch.alias(&load_loc, mem_loc);
                if result == AliasResult::NoAlias {
                    continue;
                }
                if is_load {
                    // Must aliased loads are defs of each other.
                    if result == AliasResult::MustAlias {
                        return MemDepResult::Def(inst);
                    }
                    // A partial alias is a clobber for the client to handle.
                    if let AliasResult::PartialAlias { offset: Some(offset) } = result {
                        self.clobber_offsets.insert(inst, offset);
                        return MemDepResult::Clobber(inst);
                    }
                    // Random may-alias loads don't depend on each other.
                    continue;
                }
                // Stores don't alias loads from read-only memory.
                if !batch.get_mod_ref_info_mask(&load_loc, false).is_mod_set() {
                    continue;
                }
                // Stores depend on may/must aliased loads.
                return MemDepResult::Def(inst);
            }
            if matches!(function.instruction(inst).opcode, Opcode::Store { .. }) {
                // A volatile access need not clobber non-aliasing locations.
                if is_volatile(function, inst) && query.is_none_or(|query| is_volatile(function, query)) {
                    return MemDepResult::Clobber(inst);
                }
                // If alias analysis can tell this store doesn't modify the
                // query pointer, ignore it; `getModRefInfo`
                // also handles constant memory.
                if batch.get_mod_ref_info(inst, Some(mem_loc)).is_no_mod_ref() {
                    continue;
                }
                // It might clobber the query pointer: a must alias is a def.
                let store_loc = MemoryLocation::get(unit, inst).expect("a store");
                let result = batch.alias(&store_loc, mem_loc);
                if result == AliasResult::NoAlias {
                    continue;
                }
                if result == AliasResult::MustAlias {
                    return MemDepResult::Def(inst);
                }
                if is_invariant_load {
                    continue;
                }
                if is_store_preserving_memory_location(unit, inst, mem_loc, mem_loc_align, batch, *limit) {
                    continue;
                }
                return MemDepResult::Clobber(inst);
            }
            // An allocation the accessed pointer is to: a Def, so a load of it
            // can become undef. BasicAA bypasses the allocation itself.
            if matches!(function.instruction(inst).opcode, Opcode::Alloca { .. })
                || llrm_mir::memory::returns_noalias(unit.context, unit.globals, function, inst)
            {
                let (access_ptr, _) = valuetracking::underlying(unit.context, unit.layout, function, mem_loc.ptr);
                let this = function.instruction(inst).result.map(Operand::Value);
                if this == Some(access_ptr)
                    || this.is_some_and(|this| {
                        batch.is_must_alias(
                            &MemoryLocation::before_or_after(this),
                            &MemoryLocation::before_or_after(access_ptr),
                        )
                    })
                {
                    return MemDepResult::Def(inst);
                }
            }
            // A select that is the pointer of `mem_loc` is a Def.
            if matches!(function.instruction(inst).opcode, Opcode::Select)
                && function.instruction(inst).result.map(Operand::Value) == Some(mem_loc.ptr)
            {
                return MemDepResult::Def(inst);
            }
            if is_invariant_load {
                continue;
            }
            // Something else (a call): does it mod/ref the pointer?
            match batch.get_mod_ref_info(inst, Some(mem_loc)) {
                // No effect on the queried pointer: ignore it.
                ModRefInfo::NoModRef => continue,
                ModRefInfo::Mod => return MemDepResult::Clobber(inst),
                // Never stores to the pointer: a load query scans past it.
                ModRefInfo::Ref if is_load => continue,
                // Otherwise a potential dependence.
                _ => return MemDepResult::Clobber(inst),
            }
        }
        // No dependence found. In the entry block of the function it is
        // unknown, otherwise non-local.
        if Some(bb) != function.entry() { MemDepResult::NonLocal } else { MemDepResult::NonFuncLocal }
    }

    /// `getDependency`
    /// ([:629](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L629)):
    /// the dependence of `query` within its block, from the cache where it is
    /// not dirty.
    pub fn get_dependency<A: AAResults>(
        &mut self,
        env: Env,
        aa: &mut A,
        query: InstId,
    ) -> MemDepResult {
        let (unit, function) = (env.unit, env.unit.function);
        self.clobber_offsets.clear();
        let mut scan_pos = query;
        // A cached result that is not dirty is returned (an unseen query is
        // dirty).
        let cached = self.local_deps.get_or_insert_with(query, MemDepResult::default);
        if !cached.is_dirty() {
            return *cached;
        }
        // A dirty answer holding an instruction rescans from it.
        let dirty = *cached;
        if let Some(inst) = dirty.inst() {
            scan_pos = inst;
            remove_from_reverse_map(&mut self.reverse_local_deps, inst, query);
        }
        let parent = function.parent(query).expect("a placed query");
        let result = if function.prev_in_block(query).is_none() {
            if Some(parent) != function.entry() { MemDepResult::NonLocal } else { MemDepResult::NonFuncLocal }
        } else {
            let (mem_loc, mr) = get_location(unit, query);
            if let Some(mem_loc) = mem_loc {
                let mut is_load = !mr.is_mod_set();
                is_load |= unit.intrinsic(query) == Some(Intrinsic::LifetimeStart);
                self.get_pointer_dependency_from(env, aa, &mem_loc, is_load, Some(scan_pos), parent, Some(query), None)
            } else if matches!(function.instruction(query).opcode, Opcode::Call(_)) {
                let is_read_only = BatchAAResults::new(aa).only_reads_memory(query);
                self.get_call_dependency_from(env, aa, query, is_read_only, Some(scan_pos), parent)
            } else {
                MemDepResult::Unknown
            }
        };
        self.local_deps.insert(query, result);
        // Remember the reverse map so `remove_instruction` finds who depends.
        if let Some(inst) = result.inst() {
            self.reverse_local_deps.get_or_insert_with(inst, Default::default).insert(query);
        }
        result
    }

    /// `getNonLocalCallDependency`
    /// ([:700](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L700)):
    /// the dependence of the call `query` in each block it can be reached
    /// from, as far as a non-local answer needs.
    pub fn get_non_local_call_dependency<A: AAResults>(
        &mut self,
        env: Env,
        aa: &mut A,
        query: InstId,
    ) -> &NonLocalDepInfo {
        let function = env.unit.function;
        debug_assert!(
            self.get_dependency(env, aa, query).is_non_local(),
            "getNonLocalCallDependency should only be used on calls with non-local deps!"
        );
        let (mut cache, dirty_flag) = self.non_local_deps_map.remove(&query).unwrap_or_default();
        let mut dirty_blocks: Vec<BlockId> = Vec::new();
        if !cache.is_empty() {
            // The cache is of a previous query: if nothing is dirty it stands.
            if !dirty_flag {
                self.non_local_deps_map.insert(query, (cache, dirty_flag));
                return &self.non_local_deps_map[&query].0;
            }
            // Dirty entries are where a removed instruction was depended on.
            dirty_blocks.extend(cache.iter().filter(|entry| entry.result.is_dirty()).map(|entry| entry.bb));
            cache.sort_by_key(|entry| entry.bb);
        } else {
            let query_bb = function.parent(query).expect("a placed call");
            dirty_blocks.extend(self.preds(function, query_bb));
        }
        let is_readonly_call = BatchAAResults::new(aa).only_reads_memory(query);
        let mut visited = IdSet::<BlockId>::new();
        // The entries sorted when the walk started; later ones are appended.
        let num_sorted = cache.len();
        while let Some(dirty_bb) = dirty_blocks.pop() {
            // Already looked at: the walk is over blocks, not paths.
            if !visited.insert(dirty_bb) {
                continue;
            }
            // Find the entry for the block, if the sorted part has one.
            let at = cache[..num_sorted].partition_point(|entry| entry.bb <= dirty_bb);
            let at = if at > 0 && cache[at - 1].bb == dirty_bb { at - 1 } else { at };
            let mut existing = None;
            if at != num_sorted && cache[at].bb == dirty_bb {
                // A clean entry stands.
                if !cache[at].result.is_dirty() {
                    continue;
                }
                existing = Some(at);
            }
            // Scan the block from where the dirty entry says, else from its
            // end.
            let mut scan_pos = None;
            if let Some(at) = existing
                && let Some(inst) = cache[at].result.inst()
            {
                scan_pos = Some(inst);
                remove_from_reverse_map(&mut self.reverse_non_local_deps, inst, query);
            }
            let dep = if Self::before(function, dirty_bb, scan_pos).is_some() {
                self.get_call_dependency_from(env, aa, query, is_readonly_call, scan_pos, dirty_bb)
            } else if Some(dirty_bb) != function.entry() {
                MemDepResult::NonLocal
            } else {
                MemDepResult::NonFuncLocal
            };
            match existing {
                Some(at) => cache[at].result = dep,
                None => cache.push(NonLocalDepEntry { bb: dirty_bb, result: dep }),
            }
            // A non-local answer asks the predecessors; a local one is
            // remembered.
            if !dep.is_non_local() {
                if let Some(inst) = dep.inst() {
                    self.reverse_non_local_deps.get_or_insert_with(inst, Default::default).insert(query);
                }
            } else {
                dirty_blocks.extend(self.preds(function, dirty_bb));
            }
        }
        self.non_local_deps_map.insert(query, (cache, dirty_flag));
        &self.non_local_deps_map[&query].0
    }

    /// `getNonLocalPointerDependency`
    /// ([:824](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L824)):
    /// the dependence of the load or store `query` in every block it can be
    /// reached from, with the address live in each.
    pub fn get_non_local_pointer_dependency<A: AAResults>(
        &mut self,
        env: Env,
        aa: &mut A,
        query: InstId,
        result: &mut Vec<NonLocalDepResult>,
    ) {
        let (unit, function) = (env.unit, env.unit.function);
        let loc = MemoryLocation::get(unit, query).expect("a load or a store");
        let is_load = matches!(function.instruction(query).opcode, Opcode::Load { .. });
        let from_bb = function.parent(query).expect("a placed query");
        result.clear();
        // Volatile accesses depend on everything.
        if is_volatile(function, query) {
            result.push(NonLocalDepResult {
                bb: from_bb,
                result: MemDepResult::Unknown,
                address: SelectAddr::Addr(Some(loc.ptr)),
            });
            return;
        }
        let address = PhiTransAddr::new(function, loc.ptr);
        self.epoch += 1;
        assert!(self.epoch > 0, "NonLocalPointerDepVisitedEpoch overflow");
        if self.visited.len() < function.block_capacity() {
            self.visited.resize(function.block_capacity(), (None, 0));
        }
        let mut batch = BatchAAResults::new(aa);
        if self.get_non_local_pointer_dep_from_bb(
            env,
            &mut batch,
            Some(query),
            &address,
            &loc,
            is_load,
            from_bb,
            result,
            true,
            false,
        ) {
            return;
        }
        result.clear();
        result.push(NonLocalDepResult {
            bb: from_bb,
            result: MemDepResult::Unknown,
            address: SelectAddr::Addr(Some(loc.ptr)),
        });
    }

    /// `setNonLocalPointerDepVisited`.
    fn set_visited(
        &mut self,
        bb: BlockId,
        value: Option<Operand>,
    ) {
        self.visited[bb.0 as usize] = (value, self.epoch);
    }

    /// `isNonLocalPointerDepVisited`.
    fn is_visited(
        &self,
        bb: BlockId,
    ) -> bool {
        self.visited[bb.0 as usize].1 == self.epoch
    }

    /// `lookupNonLocalPointerDepVisited`.
    fn lookup_visited(
        &self,
        bb: BlockId,
    ) -> Option<Operand> {
        assert!(self.is_visited(bb), "Visited value requested for unseen block");
        self.visited[bb.0 as usize].0
    }

    /// Phi translation of `pointer` from `cur` to `pred`.
    fn translate(
        &mut self,
        env: Env,
        pointer: &mut PhiTransAddr,
        cur: BlockId,
        pred: BlockId,
    ) -> Option<Operand> {
        let unit = env.unit;
        let mut phi = PhiEnv { context: unit.context, layout: unit.layout, function: unit.function, dt: Some(env.dt) };
        pointer.translate_value(&mut phi, cur, pred, false)
    }

    /// `getNonLocalInfoForBlock`
    /// ([:891](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L891)):
    /// the dependence of `loc` in `bb`, from `cache` where it is current.
    fn get_non_local_info_for_block<A: AAResults>(
        &mut self,
        env: Env,
        batch: &mut BatchAAResults<A>,
        query: Option<InstId>,
        loc: &MemoryLocation,
        is_load: bool,
        bb: BlockId,
        cache: &mut NonLocalDepInfo,
        num_sorted: usize,
    ) -> MemDepResult {
        let function = env.unit.function;
        let is_invariant_load = query
            .is_some_and(|inst| function.instruction(inst).metadata.iter().any(|(kind, _)| kind == "invariant.load"));
        // Find the entry for the block among the sorted ones.
        let at = cache[..num_sorted].partition_point(|entry| entry.bb <= bb);
        let at = if at > 0 && cache[at - 1].bb == bb { at - 1 } else { at };
        let mut existing = (at != num_sorted && cache[at].bb == bb).then_some(at);
        // An invariant load's cached result stands only if it is NonFuncLocal.
        if existing.is_some_and(|at| is_invariant_load && !cache[at].result.is_non_func_local()) {
            existing = None;
        }
        // A clean entry is the answer.
        if let Some(at) = existing
            && !cache[at].result.is_dirty()
        {
            return cache[at].result;
        }
        // A dirty one rescans from the instruction it names.
        let mut scan_pos = None;
        let key = (loc.ptr, is_load);
        if let Some(at) = existing
            && let Some(inst) = cache[at].result.inst()
        {
            assert!(function.parent(inst) == Some(bb), "Instruction invalidated?");
            scan_pos = Some(inst);
            remove_from_reverse_pairs(&mut self.reverse_non_local_ptr_deps, inst, key);
        }
        let dep = self.get_pointer_dependency_from_with(env, batch, loc, is_load, scan_pos, bb, query, None);
        // An invariant load's answer is not cached.
        if is_invariant_load {
            return dep;
        }
        match existing {
            Some(at) => cache[at].result = dep,
            None => cache.push(NonLocalDepEntry { bb, result: dep }),
        }
        // A non-local answer names no instruction.
        if !dep.is_local() {
            return dep;
        }
        let inst = dep.inst().expect("Didn't depend on anything?");
        self.reverse_non_local_ptr_deps.get_or_insert_with(inst, Default::default).insert(key);
        dep
    }

    /// `getNonLocalPointerDepFromBB`
    /// ([:1045](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1045)):
    /// the dependence of `loc` in `start_bb` and, where it is transparent, in
    /// the predecessors; false where the walk failed and the caller answers
    /// Unknown. One batch serves the recursion (DEVIATE 4).
    fn get_non_local_pointer_dep_from_bb<A: AAResults>(
        &mut self,
        env: Env,
        batch: &mut BatchAAResults<A>,
        query: Option<InstId>,
        pointer: &PhiTransAddr,
        loc: &MemoryLocation,
        is_load: bool,
        start_bb: BlockId,
        result: &mut Vec<NonLocalDepResult>,
        mut skip_first_block: bool,
        mut is_incomplete: bool,
    ) -> bool {
        let function = env.unit.function;
        let addr = pointer.addr().expect("an address");
        let cache_key: ValueIsLoadPair = (addr, is_load);
        let is_invariant_load = query
            .is_some_and(|inst| function.instruction(inst).metadata.iter().any(|(kind, _)| kind == "invariant.load"));
        let inserted = !self.non_local_pointer_deps.contains_key(&cache_key);
        self.non_local_pointer_deps.entry(cache_key).or_insert_with(|| NonLocalPointerInfo::new(loc.size, loc.aa_info));
        if !is_invariant_load && !inserted {
            // The cached answers must be of this size and these tags.
            let info = self.non_local_pointer_deps.get_mut(&cache_key).expect("inserted");
            if info.size != loc.size {
                info.pair = (None, false);
                info.size = loc.size;
                let dropped = std::mem::take(&mut info.non_local_deps);
                for entry in &dropped {
                    if let Some(inst) = entry.result.inst() {
                        remove_from_reverse_pairs(&mut self.reverse_non_local_ptr_deps, inst, cache_key);
                    }
                }
                is_incomplete = true;
            }
            let info = self.non_local_pointer_deps.get_mut(&cache_key).expect("inserted");
            if info.aa_tags != loc.aa_info {
                if info.aa_tags != AAInfo::default() {
                    info.pair = (None, false);
                    info.aa_tags = AAInfo::default();
                    let dropped = std::mem::take(&mut info.non_local_deps);
                    for entry in &dropped {
                        if let Some(inst) = entry.result.inst() {
                            remove_from_reverse_pairs(&mut self.reverse_non_local_ptr_deps, inst, cache_key);
                        }
                    }
                    is_incomplete = true;
                }
                if loc.aa_info != AAInfo::default() {
                    let untagged = MemoryLocation { aa_info: AAInfo::default(), ..*loc };
                    return self.get_non_local_pointer_dep_from_bb(
                        env,
                        batch,
                        query,
                        pointer,
                        &untagged,
                        is_load,
                        start_bb,
                        result,
                        skip_first_block,
                        is_incomplete,
                    );
                }
            }
        }
        // A complete cache of this very query answers it.
        let info = &self.non_local_pointer_deps[&cache_key];
        if !is_incomplete && !is_invariant_load && info.pair == (Some(start_bb), skip_first_block) {
            let entries = info.non_local_deps.clone();
            // The pointer each block was reached with this query must be this
            // one.
            for entry in &entries {
                if !self.is_visited(entry.bb) {
                    continue;
                }
                if self.lookup_visited(entry.bb) == Some(addr) {
                    continue;
                }
                return false;
            }
            for entry in &entries {
                self.set_visited(entry.bb, Some(addr));
                // A non-local entry is a block the answer passes through.
                if entry.result.is_non_local() {
                    continue;
                }
                if env.dt.is_reachable_from_entry(entry.bb) {
                    result.push(NonLocalDepResult {
                        bb: entry.bb,
                        result: entry.result,
                        address: SelectAddr::Addr(Some(addr)),
                    });
                }
            }
            return true;
        }
        // Bound the cache.
        if self.non_local_pointer_deps[&cache_key].non_local_deps.len() > CACHE_GLOBAL_LIMIT {
            return false;
        }
        // These answers are of this query, if it started from nothing.
        if !is_invariant_load {
            let info = self.non_local_pointer_deps.get_mut(&cache_key).expect("inserted");
            info.pair = if !is_incomplete && info.non_local_deps.is_empty() {
                (Some(start_bb), skip_first_block)
            } else {
                (None, false)
            };
        }
        let mut worklist = vec![start_bb];
        let mut pred_list: Vec<(BlockId, PhiTransAddr)> = Vec::new();
        // `Cache` is the live cache of `cache_key`, or none while a recursion
        // may have changed it; the sorted prefix is what binary search sees.
        let mut cache_live = true;
        let mut num_sorted = self.non_local_pointer_deps[&cache_key].non_local_deps.len();
        let mut worklist_entries = BLOCK_NUMBER_LIMIT;
        let mut got_worklist_limit = false;
        'blocks: while let Some(bb) = worklist.pop() {
            // Too many results: give up and cache nothing as complete.
            if result.len() > NUM_RESULTS_LIMIT {
                if cache_live {
                    let cache = &mut self.non_local_pointer_deps.get_mut(&cache_key).expect("inserted").non_local_deps;
                    if num_sorted != cache.len() {
                        sort_non_local_dep_info_cache(cache, num_sorted);
                    }
                }
                self.non_local_pointer_deps.get_mut(&cache_key).expect("inserted").pair = (None, false);
                return false;
            }
            // The block's own answer, unless the caller asked to skip it.
            'translate: {
                if !skip_first_block {
                    assert!(self.is_visited(bb), "Should check 'visited' before adding to WL");
                    let mut cache = std::mem::take(
                        &mut self.non_local_pointer_deps.get_mut(&cache_key).expect("inserted").non_local_deps,
                    );
                    let dep =
                        self.get_non_local_info_for_block(env, batch, query, loc, is_load, bb, &mut cache, num_sorted);
                    self.non_local_pointer_deps.get_mut(&cache_key).expect("inserted").non_local_deps = cache;
                    // A local answer ends this path; an unreachable block's
                    // does not.
                    if !dep.is_non_local() && env.dt.is_reachable_from_entry(bb) {
                        result.push(NonLocalDepResult { bb, result: dep, address: SelectAddr::Addr(Some(addr)) });
                        continue 'blocks;
                    }
                }
                // Without a phi to translate through, the predecessors take the
                // same pointer.
                if !pointer.needs_phi_translation_from_block(function, bb) {
                    skip_first_block = false;
                    let mut new_blocks: Vec<BlockId> = Vec::new();
                    for pred in self.preds(function, bb) {
                        if !self.is_visited(pred) {
                            self.set_visited(pred, Some(addr));
                            new_blocks.push(pred);
                            continue;
                        }
                        // Reached before with another pointer: give up on the
                        // block.
                        if self.lookup_visited(pred) != Some(addr) {
                            for &new_block in &new_blocks {
                                self.set_visited(new_block, None);
                            }
                            break 'translate;
                        }
                    }
                    if new_blocks.len() > worklist_entries {
                        for &new_block in &new_blocks {
                            self.set_visited(new_block, None);
                        }
                        got_worklist_limit = true;
                        break 'translate;
                    }
                    worklist_entries -= new_blocks.len();
                    worklist.extend(new_blocks);
                    continue 'blocks;
                }
                // A pointer that is not phi translatable cannot continue.
                if !pointer.is_potentially_phi_translatable(env.unit.context, function) {
                    break 'translate;
                }
                // Sort before the recursion changes the cache under us.
                if cache_live {
                    let cache = &mut self.non_local_pointer_deps.get_mut(&cache_key).expect("inserted").non_local_deps;
                    if num_sorted != cache.len() {
                        sort_non_local_dep_info_cache(cache, num_sorted);
                        num_sorted = cache.len();
                    }
                }
                cache_live = false;
                pred_list.clear();
                for pred in self.preds(function, bb) {
                    pred_list.push((pred, pointer.clone()));
                    let mut translated = pred_list.pop().expect("just pushed");
                    let pred_ptr_val = self.translate(env, &mut translated.1, bb, pred);
                    if !self.is_visited(pred) {
                        self.set_visited(pred, pred_ptr_val);
                        pred_list.push(translated);
                        continue;
                    }
                    let prev_val = self.lookup_visited(pred);
                    if prev_val == pred_ptr_val {
                        continue;
                    }
                    // Reached with another pointer: forget what this block set.
                    for (listed, _) in &pred_list {
                        self.visited[listed.0 as usize].0 = None;
                    }
                    break 'translate;
                }
                // Look up each predecessor's pointer.
                for (pred, pred_pointer) in std::mem::take(&mut pred_list) {
                    let pred_ptr_val = pred_pointer.addr();
                    let mut can_translate = true;
                    if pred_ptr_val.is_none() {
                        // A select on a condition has two addresses, one per
                        // side.
                        if let Some(cond) = pred_pointer.select_condition(function) {
                            let mut phi = PhiEnv {
                                context: env.unit.context,
                                layout: env.unit.layout,
                                function,
                                dt: Some(env.dt),
                            };
                            let sides: SelectAddrs = pointer.translate_value_select(&mut phi, bb, pred, cond);
                            if let (Some(on_true), Some(on_false)) = sides {
                                result.push(NonLocalDepResult {
                                    bb: pred,
                                    result: MemDepResult::Select,
                                    address: SelectAddr::Select { cond, addrs: (on_true, on_false) },
                                });
                                self.non_local_pointer_deps
                                    .entry(cache_key)
                                    .or_insert_with(|| NonLocalPointerInfo::new(loc.size, loc.aa_info))
                                    .pair = (None, false);
                                continue;
                            }
                        }
                        can_translate = false;
                    }
                    if !can_translate
                        || !self.get_non_local_pointer_dep_from_bb(
                            env,
                            batch,
                            query,
                            &pred_pointer,
                            &loc.with_new_ptr(pred_ptr_val.expect("translated")),
                            is_load,
                            pred,
                            result,
                            false,
                            false,
                        )
                    {
                        result.push(NonLocalDepResult {
                            bb: pred,
                            result: MemDepResult::Unknown,
                            address: SelectAddr::Addr(pred_ptr_val),
                        });
                        self.non_local_pointer_deps
                            .entry(cache_key)
                            .or_insert_with(|| NonLocalPointerInfo::new(loc.size, loc.aa_info))
                            .pair = (None, false);
                        continue;
                    }
                }
                // The cache may have been rehashed by the recursion: take it
                // again.
                let info = self
                    .non_local_pointer_deps
                    .entry(cache_key)
                    .or_insert_with(|| NonLocalPointerInfo::new(loc.size, loc.aa_info));
                cache_live = true;
                num_sorted = info.non_local_deps.len();
                info.pair = (None, false);
                skip_first_block = false;
                continue 'blocks;
            }
            // PredTranslationFailure: this block's answer is Unknown.
            if !cache_live {
                cache_live = true;
                num_sorted = self
                    .non_local_pointer_deps
                    .entry(cache_key)
                    .or_insert_with(|| NonLocalPointerInfo::new(loc.size, loc.aa_info))
                    .non_local_deps
                    .len();
            }
            let info = self.non_local_pointer_deps.get_mut(&cache_key).expect("inserted");
            info.pair = (None, false);
            // The first block's failure is the caller's to answer.
            if skip_first_block {
                return false;
            }
            if !is_invariant_load {
                for entry in info.non_local_deps.iter_mut().rev() {
                    if entry.bb != bb {
                        continue;
                    }
                    assert!(
                        got_worklist_limit || entry.result.is_non_local() || !env.dt.is_reachable_from_entry(bb),
                        "Should only be here with transparent block"
                    );
                    entry.result = MemDepResult::Unknown;
                    break;
                }
            }
            result.push(NonLocalDepResult { bb, result: MemDepResult::Unknown, address: SelectAddr::Addr(Some(addr)) });
        }
        let cache = &mut self.non_local_pointer_deps.get_mut(&cache_key).expect("inserted").non_local_deps;
        sort_non_local_dep_info_cache(cache, num_sorted);
        true
    }

    /// `removeCachedNonLocalPointerDependencies`
    /// ([:1461](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1461)).
    fn remove_cached_non_local_pointer_dependencies(
        &mut self,
        function: &Function,
        pair: ValueIsLoadPair,
    ) {
        let Some(info) = self.non_local_pointer_deps.remove(&pair) else { return };
        for entry in &info.non_local_deps {
            // Ignore non-local results.
            let Some(target) = entry.result.inst() else { continue };
            assert!(function.parent(target) == Some(entry.bb));
            remove_from_reverse_pairs(&mut self.reverse_non_local_ptr_deps, target, pair);
        }
    }

    /// `invalidateCachedPointerInfo`
    /// ([:1507](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1507)):
    /// forget what is known of `ptr`, because it changed.
    pub fn invalidate_cached_pointer_info(
        &mut self,
        unit: &Unit,
        ptr: Operand,
    ) {
        let is_pointer = unit
            .function
            .operand_type(unit.context, ptr)
            .is_some_and(|ty| matches!(unit.context.types.get(ty), llrm_mir::types::Type::Pointer(_)));
        if !is_pointer {
            return;
        }
        self.remove_cached_non_local_pointer_dependencies(unit.function, (ptr, false));
        self.remove_cached_non_local_pointer_dependencies(unit.function, (ptr, true));
    }

    /// `removeInstruction`
    /// ([:1521](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1521)):
    /// forget `rem`, called while it is still in its block. What depended on it
    /// is dirtied to the instruction after it.
    pub fn remove_instruction(
        &mut self,
        unit: &Unit,
        rem: InstId,
    ) {
        let function = unit.function;
        // Drop its own non-local call dependencies.
        if let Some((block_map, _)) = self.non_local_deps_map.remove(&rem) {
            for entry in &block_map {
                if let Some(inst) = entry.result.inst() {
                    remove_from_reverse_map(&mut self.reverse_non_local_deps, inst, rem);
                }
            }
        }
        // And its local dependency.
        if let Some(dep) = self.local_deps.remove(&rem)
            && let Some(inst) = dep.inst()
        {
            remove_from_reverse_map(&mut self.reverse_local_deps, inst, rem);
        }
        // A pointer: what is cached of it, as a read and as a write.
        if let Some(result) = function.instruction(rem).result
            && matches!(
                unit.context.types.get(function.instruction(rem).ty),
                llrm_mir::types::Type::Pointer(_)
            )
        {
            self.remove_cached_non_local_pointer_dependencies(function, (Operand::Value(result), false));
            self.remove_cached_non_local_pointer_dependencies(function, (Operand::Value(result), true));
        }
        // What depended on `rem` now depends on what follows it (dirty), or
        // nothing for a terminator.
        let mut reverse_deps_to_add: Vec<(InstId, InstId)> = Vec::new();
        let new_dirty_val = if !function.instruction(rem).opcode.is_terminator() {
            MemDepResult::Invalid(function.next_in_block(rem))
        } else {
            MemDepResult::default()
        };
        if let Some(depending) = self.reverse_local_deps.remove(&rem) {
            assert!(
                !depending.is_empty() && !function.instruction(rem).opcode.is_terminator(),
                "Nothing can locally depend on a terminator"
            );
            for inst in depending.iter() {
                assert!(inst != rem, "Already removed our local dep info");
                self.local_deps.insert(inst, new_dirty_val);
                let next = new_dirty_val
                    .inst()
                    .expect("There is no way something else can have a local dep on this if it is a terminator!");
                reverse_deps_to_add.push((next, inst));
            }
            while let Some((key, value)) = reverse_deps_to_add.pop() {
                self.reverse_local_deps.get_or_insert_with(key, Default::default).insert(value);
            }
        }
        if let Some(depending) = self.reverse_non_local_deps.remove(&rem) {
            for inst in depending.iter() {
                assert!(inst != rem, "Already removed NonLocalDep info for RemInst");
                let info = self.non_local_deps_map.get_or_insert_with(inst, Default::default);
                info.1 = true;
                for entry in &mut info.0 {
                    if entry.result.inst() != Some(rem) {
                        continue;
                    }
                    entry.result = new_dirty_val;
                    if let Some(next) = new_dirty_val.inst() {
                        reverse_deps_to_add.push((next, inst));
                    }
                }
            }
            while let Some((key, value)) = reverse_deps_to_add.pop() {
                self.reverse_non_local_deps.get_or_insert_with(key, Default::default).insert(value);
            }
        }
        if let Some(pairs) = self.reverse_non_local_ptr_deps.remove(&rem) {
            let mut to_add: Vec<(InstId, ValueIsLoadPair)> = Vec::new();
            for pair in pairs {
                assert!(
                    function.instruction(rem).result.map(Operand::Value) != Some(pair.0),
                    "Already removed NonLocalPointerDeps info for RemInst"
                );
                let info = self
                    .non_local_pointer_deps
                    .entry(pair)
                    .or_insert_with(|| NonLocalPointerInfo::new(LocationSize::AfterPointer, AAInfo::default()));
                info.pair = (None, false);
                for entry in &mut info.non_local_deps {
                    if entry.result.inst() != Some(rem) {
                        continue;
                    }
                    entry.result = new_dirty_val;
                    if let Some(next) = new_dirty_val.inst() {
                        to_add.push((next, pair));
                    }
                }
                info.non_local_deps.sort_by_key(|entry| entry.bb);
            }
            for (key, pair) in to_add {
                self.reverse_non_local_ptr_deps.get_or_insert_with(key, Default::default).insert(pair);
            }
        }
        assert!(!self.non_local_deps_map.contains_key(&rem), "RemInst got reinserted?");
    }

    /// `verifyRemoved`
    /// ([:1705](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1705)):
    /// panics where `removed` is still named by a cache.
    pub fn verify_removed(
        &self,
        function: &Function,
        removed: InstId,
    ) {
        let value =
            function.instruction(removed).result.map(Operand::Value).unwrap_or(Operand::Block(BlockId(u32::MAX)));
        for (key, dep) in self.local_deps.iter() {
            assert!(key != removed && dep.inst() != Some(removed), "Inst occurs in data structures");
        }
        for (key, (entries, _)) in self.non_local_deps_map.iter() {
            assert!(key != removed, "Inst occurs in data structures");
            for entry in entries {
                assert!(entry.result.inst() != Some(removed), "Inst occurs in data structures");
            }
        }
        for (key, set) in self.reverse_local_deps.iter().chain(self.reverse_non_local_deps.iter()) {
            assert!(key != removed && !set.contains(&removed), "Inst occurs in data structures");
        }
        for (&(pointer, _), info) in &self.non_local_pointer_deps {
            assert!(pointer != value, "Inst occurs in NLPD map key");
            for entry in &info.non_local_deps {
                assert!(entry.result.inst() != Some(removed), "Inst occurs as NLPD value");
            }
        }
        for (key, set) in self.reverse_non_local_ptr_deps.iter() {
            assert!(key != removed, "Inst occurs in rev NLPD map");
            for &(pointer, _) in set {
                assert!(pointer != value, "Inst occurs in ReverseNonLocalPtrDeps map");
            }
        }
    }
}

/// `SortNonLocalDepInfoCache`
/// ([:960](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L960)):
/// the cache by block, where `num_sorted` of it already is.
fn sort_non_local_dep_info_cache(
    cache: &mut NonLocalDepInfo,
    num_sorted: usize,
) {
    if cache.len() < 2 {
        return;
    }
    let mut unsorted = cache.len() - num_sorted;
    if unsorted == 0 {
        return;
    }
    if num_sorted == 0 {
        cache.sort_by_key(|entry| entry.bb);
        return;
    }
    // Few entries to place: insert each (Log2_32 is floor of the log).
    if unsorted < (cache.len() as u32).ilog2() as usize {
        while unsorted > 0 {
            let value = cache.pop().expect("an unsorted entry");
            let end = cache.len() - unsorted + 1;
            let at = cache[..end].partition_point(|entry| entry.bb <= value.bb);
            cache.insert(at, value);
            unsorted -= 1;
        }
    } else {
        cache.sort_by_key(|entry| entry.bb);
    }
}

/// `Instruction::isVolatile`.
fn is_volatile(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(
        function.instruction(inst).opcode,
        Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. }
    )
}

#[cfg(test)]
#[path = "memdep_tests.rs"]
mod tests;
