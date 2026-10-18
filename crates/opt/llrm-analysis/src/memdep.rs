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
//! `get_non_local_call_dependency`, `get_pointer_dependency_from`,
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

use llrm_mir::facts::Facts;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Opcode, Tail};
use llrm_mir::valuetracking::{self, Query};
use llrm_support::hash::{HashMap, HashSet};

use crate::aa::{AAResults, AliasResult, BatchAAResults, MemoryLocation, ModRefInfo};
use crate::effects;
use crate::memory::Unit;

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

/// What a scan reads: the function and its module's types and attributes.
#[derive(Clone, Copy)]
pub struct Env<'a> {
    pub unit: &'a Unit<'a>,
}

/// [`RemoveFromReverseMap`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L96).
fn remove_from_reverse_map<K: Copy + Eq + std::hash::Hash>(
    reverse: &mut HashMap<InstId, HashSet<K>>,
    inst: InstId,
    value: K,
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

/// `isNoAliasCall`: a call whose result is `noalias`.
fn is_no_alias_call(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(
        &function.instruction(inst).opcode,
        Opcode::Call(info) if Facts::of(&info.return_attrs).no_alias()
    )
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
    local_deps: HashMap<InstId, MemDepResult>,
    non_local_deps_map: HashMap<InstId, (NonLocalDepInfo, bool)>,
    reverse_local_deps: HashMap<InstId, HashSet<InstId>>,
    reverse_non_local_deps: HashMap<InstId, HashSet<InstId>>,
    pred_cache: HashMap<BlockId, Vec<BlockId>>,
    clobber_offsets: HashMap<InstId, i32>,
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
            local_deps: HashMap::default(),
            non_local_deps_map: HashMap::default(),
            reverse_local_deps: HashMap::default(),
            reverse_non_local_deps: HashMap::default(),
            pred_cache: HashMap::default(),
            clobber_offsets: HashMap::default(),
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
        self.pred_cache.entry(bb).or_insert_with(|| function.predecessors(bb).to_vec()).clone()
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
            None => function.block(bb).instructions().last().copied(),
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
            if matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }) || is_no_alias_call(function, inst) {
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
        let cached = self.local_deps.entry(query).or_default();
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
            self.reverse_local_deps.entry(inst).or_default().insert(query);
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
        let mut visited: HashSet<BlockId> = HashSet::default();
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
                    self.reverse_non_local_deps.entry(inst).or_default().insert(query);
                }
            } else {
                dirty_blocks.extend(self.preds(function, dirty_bb));
            }
        }
        self.non_local_deps_map.insert(query, (cache, dirty_flag));
        &self.non_local_deps_map[&query].0
    }

    /// `removeInstruction`
    /// ([:1521](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1521)):
    /// forget `rem`, called while it is still in its block. What depended on it
    /// is dirtied to the instruction after it.
    pub fn remove_instruction(
        &mut self,
        function: &Function,
        rem: InstId,
    ) {
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
            for inst in depending {
                assert!(inst != rem, "Already removed our local dep info");
                self.local_deps.insert(inst, new_dirty_val);
                let next = new_dirty_val
                    .inst()
                    .expect("There is no way something else can have a local dep on this if it is a terminator!");
                reverse_deps_to_add.push((next, inst));
            }
            while let Some((key, value)) = reverse_deps_to_add.pop() {
                self.reverse_local_deps.entry(key).or_default().insert(value);
            }
        }
        if let Some(depending) = self.reverse_non_local_deps.remove(&rem) {
            for inst in depending {
                assert!(inst != rem, "Already removed NonLocalDep info for RemInst");
                let info = self.non_local_deps_map.entry(inst).or_default();
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
                self.reverse_non_local_deps.entry(key).or_default().insert(value);
            }
        }
        assert!(!self.non_local_deps_map.contains_key(&rem), "RemInst got reinserted?");
    }

    /// `verifyRemoved`
    /// ([:1705](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1705)):
    /// panics where `removed` is still named by a cache.
    pub fn verify_removed(
        &self,
        removed: InstId,
    ) {
        for (&key, dep) in &self.local_deps {
            assert!(key != removed && dep.inst() != Some(removed), "Inst occurs in data structures");
        }
        for (&key, (entries, _)) in &self.non_local_deps_map {
            assert!(key != removed, "Inst occurs in data structures");
            for entry in entries {
                assert!(entry.result.inst() != Some(removed), "Inst occurs in data structures");
            }
        }
        for (key, set) in self.reverse_local_deps.iter().chain(self.reverse_non_local_deps.iter()) {
            assert!(*key != removed && !set.contains(&removed), "Inst occurs in data structures");
        }
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
