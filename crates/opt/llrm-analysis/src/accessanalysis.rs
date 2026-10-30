//! `AccessAnalysis`: the memory accesses of a loop, grouped by what they may
//! alias, for the dependence checker and the runtime checks.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`. The loads
//! and stores are added with the locations they touch across all iterations;
//! an `AliasSetTracker` over a batch alias analysis in cross-iteration mode
//! partitions them. `buildDependenceSets` joins the accesses of one set that
//! share an underlying object into dependence classes (`DepCands`) and lists
//! those that need a dependence check; `canCheckPtrAtRT` finds the bounds of
//! each pointer of each set and fills the `RuntimePointerChecking`.
//!
//! # Example
//! `a[i] = b[i]` over `a` and `b` that may alias: one set of two pointers, two
//! dependence classes, so one check that `[a, a + 4n)` and `[b, b + 4n)` do
//! not meet.
//!
//! # DEVIATE 24 (no predicated SCEV)
//! `createCheckForAccess` in its `Assume` retry would add `PSE.getAsAddRec`
//! and wrap predicates; here a pointer that is not an add recurrence, or may
//! wrap, cannot be checked.
//!
//! # DEVIATE 26 (no alias scope lists)
//! MIR's `AAInfo` has the TBAA tag and one `noalias` scope but no
//! `alias.scope` list and no `llvm.experimental.noalias.scope.decl` scopes
//! declared in a loop (`adjustAliasScopeList`): the locations are added as
//! they are.

use std::cell::RefCell;
use std::rc::Rc;

use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::types::TypeId;
use llrm_mir::valuetracking::underlying::{MAX_LOOKUP_SEARCH_DEPTH, get_underlying_objects};
use llrm_support::hash::HashMap;

use crate::aa::{AAResults, AliasResult, BatchAAResults, LocationSize, MemoryLocation};
use crate::aliassets::{AliasLocation, AliasLocations, AliasSetTracker};
use crate::loopaccess::{
    SymbolicStrideMap, find_forked_scevs, is_no_wrap, load_store_pointer_operand, null_pointer_is_defined,
    replace_symbolic_stride_scev,
};
use crate::memdepchecker::{DepType, MemoryDepChecker};
use crate::mssa::alias::ModRef;
use crate::runtimecheck::{DepCandidates, MemAccessInfo, RuntimePointerChecking, get_non_affine_monotonic_bounds};
use crate::scev::expr::{Kind, Scev, Ty};
use crate::scev::{Cx, ScalarEvolution};

/// `-max-forked-scev-depth`.
const MAX_FORKED_SCEV_DEPTH: u32 = 5;

impl AliasLocation for MemoryLocation {
    fn pointer(&self) -> Option<Operand> {
        Some(self.ptr)
    }

    fn is_same(
        &self,
        other: &Self,
    ) -> bool {
        self == other
    }
}

/// The batch alias analysis, as the alias sets query it.
struct BatchLocations<'a, 'b, A: AAResults>(&'a RefCell<BatchAAResults<'b, A>>);

impl<A: AAResults> AliasLocations for BatchLocations<'_, '_, A> {
    type Loc = MemoryLocation;

    fn alias(
        &self,
        one: &MemoryLocation,
        other: &MemoryLocation,
    ) -> crate::mssa::alias::AliasResult {
        match self.0.borrow_mut().alias(one, other) {
            AliasResult::NoAlias => crate::mssa::alias::AliasResult::No,
            AliasResult::MayAlias => crate::mssa::alias::AliasResult::May,
            AliasResult::PartialAlias { .. } => crate::mssa::alias::AliasResult::Partial,
            AliasResult::MustAlias => crate::mssa::alias::AliasResult::Must,
        }
    }

    /// Only `aliasesUnknownInst` asks, and the tracker is not given unknown
    /// instructions here.
    fn get_mod_ref_info(
        &self,
        _: InstId,
        _: &MemoryLocation,
    ) -> ModRef {
        ModRef { reads: true, writes: true }
    }

    fn without_aa_tags(
        &self,
        location: &Rc<MemoryLocation>,
    ) -> Rc<MemoryLocation> {
        Rc::new(MemoryLocation { aa_info: Default::default(), ..**location })
    }
}

/// `MapVector<MemAccessInfo, SmallSetVector<Type *, 1>>`: the accesses in the
/// order they were first added, each with the types it was accessed as.
#[derive(Clone, Debug, Default)]
struct PtrAccessMap {
    order: Vec<MemAccessInfo>,
    types: HashMap<MemAccessInfo, Vec<TypeId>>,
}

impl PtrAccessMap {
    fn insert(
        &mut self,
        access: MemAccessInfo,
        ty: Option<TypeId>,
    ) {
        let entry = self
            .types
            .entry(access)
            .or_insert_with(
                || {
                    self.order.push(access);
                    Vec::new()
                },
            );
        if let Some(ty) = ty
            && !entry.contains(&ty)
        {
            entry.push(ty);
        }
    }

    fn contains(
        &self,
        access: &MemAccessInfo,
    ) -> bool {
        self.types.contains_key(access)
    }
}

/// `AccessAnalysis`.
pub struct AccessAnalysis<'a, A: AAResults> {
    /// Map of all accesses. Values are the types used to access memory pointed
    /// to by the pointer.
    accesses: PtrAccessMap,
    /// The loop being checked (its header).
    the_loop: BlockId,
    /// List of accesses that need a further dependence check.
    check_deps: Vec<MemAccessInfo>,
    /// Set of pointers that are read only.
    read_only_ptr: Vec<Operand>,
    /// Batched alias analysis results, in cross-iteration mode.
    baa: RefCell<BatchAAResults<'a, A>>,
    /// The locations the alias sets partition, in the order they were added.
    locations: Vec<MemoryLocation>,
    /// Initial processing of memory accesses determined that we may need to
    /// add memchecks. Perform the analysis to determine the necessary checks.
    is_rt_check_analysis_needed: bool,
    /// The loop's backedge-taken count and its symbolic maximum.
    btc: Scev,
    symbolic_max_btc: Scev,
}

impl<'a, A: AAResults> AccessAnalysis<'a, A> {
    /// `AccessAnalysis(TheLoop, AA, LI, DT, DA, PSE, LoopAliasScopes)`.
    pub fn new(
        the_loop: BlockId,
        aa: &'a mut A,
        btc: Scev,
        symbolic_max_btc: Scev,
    ) -> Self {
        let mut baa = BatchAAResults::new(aa);
        // We're analyzing dependences across loop iterations.
        baa.enable_cross_iteration_mode();
        Self {
            accesses: PtrAccessMap::default(),
            the_loop,
            check_deps: Vec::new(),
            read_only_ptr: Vec::new(),
            baa: RefCell::new(baa),
            locations: Vec::new(),
            is_rt_check_analysis_needed: false,
            btc,
            symbolic_max_btc,
        }
    }

    /// `adjustLoc`: the location accessed across all iterations, rather than
    /// a single one. The accessed location varies within the loop, but remains
    /// within the underlying object.
    fn adjust_loc(mut loc: MemoryLocation) -> MemoryLocation {
        loc.size = LocationSize::BeforeOrAfterPointer;
        loc
    }

    /// `addLoad`: register a load and whether it is only read from.
    pub fn add_load(
        &mut self,
        loc: MemoryLocation,
        access_ty: TypeId,
        is_read_only: bool,
    ) {
        let ptr = loc.ptr;
        self.locations.push(Self::adjust_loc(loc));
        self.accesses.insert((ptr, false), Some(access_ty));
        if is_read_only && !self.read_only_ptr.contains(&ptr) {
            self.read_only_ptr.push(ptr);
        }
    }

    /// `addStore`: register a store.
    pub fn add_store(
        &mut self,
        loc: MemoryLocation,
        access_ty: TypeId,
    ) {
        let ptr = loc.ptr;
        self.locations.push(Self::adjust_loc(loc));
        self.accesses.insert((ptr, true), Some(access_ty));
    }

    /// `isDependencyCheckNeeded`: initial processing of memory accesses
    /// determined that we need to perform dependency checking. (This can
    /// later be cleared if we retry memcheck analysis without dependency
    /// checking.)
    pub fn is_dependency_check_needed(&self) -> bool {
        !self.check_deps.is_empty()
    }

    /// `resetDepChecks`: we decided that no dependence analysis would be
    /// used. Reset the state.
    pub fn reset_dep_checks(
        &mut self,
        dep_checker: &mut MemoryDepChecker,
    ) {
        self.check_deps.clear();
        dep_checker.clear_dependences();
    }

    /// `getDependenciesToCheck`.
    pub fn dependencies_to_check(&self) -> &[MemAccessInfo] {
        &self.check_deps
    }

    /// The pointers of each alias set, in the order of the sets and, within a
    /// set, of the locations (`for (const auto &AS : AST) AS.getPointers()`).
    fn alias_set_pointers(&self) -> Vec<Vec<Operand>> {
        let adapter = BatchLocations(&self.baa);
        let mut tracker = AliasSetTracker::new(&adapter);
        for location in &self.locations {
            // `AST.add(Loc)`: `addMemoryLocation(Loc, ModRefInfo::ModRef)`.
            tracker.add_memory_location(&Rc::new(*location), ModRef { reads: true, writes: true });
        }
        tracker.alias_sets().map(|(_, set)| set.get_pointers()).collect()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1346
    /// `createCheckForAccess`: whether a run-time no-alias check for `access`
    /// can be emitted. If so, this also adds it to a dependence set and adds a
    /// run-time check for it to `rt_check`. If `assume` is true, we will
    /// attempt to use additional run-time checks in order to get the bounds of
    /// the pointer.
    #[allow(clippy::too_many_arguments)]
    fn create_check_for_access(
        &self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        rt_check: &mut RuntimePointerChecking,
        dep_cands: &DepCandidates,
        access: MemAccessInfo,
        access_ty: TypeId,
        strides: &SymbolicStrideMap,
        dep_set_id: &mut HashMap<Operand, u32>,
        running_dep_id: &mut u32,
        as_id: u32,
        assume: bool,
    ) -> bool {
        let lp = self.the_loop;
        let ptr = access.0;

        let mut rt_check_ptrs = find_forked_scevs(se, cx, lp, ptr, MAX_FORKED_SCEV_DEPTH);
        debug_assert!(!rt_check_ptrs.is_empty(), "Must have some runtime-check pointer candidates");

        // RTCheckPtrs must have size 2 if there are forked pointers.
        // Otherwise, there are no forked pointers; replaceSymbolicStridesSCEV
        // in this case.
        let is_loop_invariant_or_ar = |se: &mut ScalarEvolution, (scev, _): &(Scev, bool)| {
            se.is_loop_invariant(cx, *scev, Some(lp)) || matches!(se.node(*scev).kind, Kind::AddRec(..))
        };
        let forked = rt_check_ptrs.len() == 2 && rt_check_ptrs.iter().all(|one| is_loop_invariant_or_ar(se, one));
        if !forked {
            rt_check_ptrs = vec![(replace_symbolic_stride_scev(se, cx, strides, ptr), false)];
        }

        // Check whether all pointers can participate in a runtime bounds
        // check. They must either be invariant or non-wrapping affine AddRecs.
        let single = rt_check_ptrs.len() == 1;
        for fork in &mut rt_check_ptrs {
            // The bounds for loop-invariant pointer is trivial.
            if se.is_loop_invariant(cx, fork.0, Some(lp)) {
                continue;
            }

            // `PSE.getAsAddRec(Ptr, &Predicates)` finds none here (D24).
            let ar = matches!(se.node(fork.0).kind, Kind::AddRec(..)).then_some(fork.0);
            let affine = ar.is_some_and(|ar| se.is_affine(ar));
            let Some(ar) = ar.filter(|_| affine) else {
                // Check if bounds for non-affine monotonic expressions can be
                // formed.
                let Ty::Ptr(space) = se.type_of(fork.0) else { unreachable!("a pointer expression") };
                let elt_size = se.get_store_size_of_expr(cx, Ty::Int(cx.layout.pointer(space).index_bits), access_ty);
                if !assume || get_non_affine_monotonic_bounds(se, cx, lp, fork.0, elt_size).is_none() {
                    return false;
                }
                continue;
            };

            // If there's only one option for Ptr, look Ptr up again
            // afterwards.
            let mut ar = ar;
            if single
                && let stride_ar = replace_symbolic_stride_scev(se, cx, strides, ptr)
                && matches!(se.node(stride_ar).kind, Kind::AddRec(..))
            {
                ar = stride_ar;
            }
            if single {
                fork.0 = ar;
            }

            // D24: with `Assume`, LLVM adds an overflow predicate for a
            // pointer that may wrap; here it cannot be checked.
            if !is_no_wrap(se, cx, ar, single.then_some(ptr), access_ty, lp, None) {
                return false;
            }
        }

        // Remember the number of pointers inserted so far, to remove the
        // pointers of this access again if the bounds of any of them cannot be
        // computed, to avoid partial inserts.
        let num_pointers = rt_check.pointers.len();
        for &(ptr_expr, needs_freeze) in &rt_check_ptrs {
            // The id of the dependence set.
            let dep_id = if dep_cands.contains(&access) {
                let leader = dep_cands.leader_value(&access).expect("a member").0;
                let leader_id = dep_set_id.entry(leader).or_insert(0);
                if *leader_id == 0 {
                    *leader_id = *running_dep_id;
                    *running_dep_id += 1;
                }
                *leader_id
            } else {
                // Each access has its own dependence set.
                let id = *running_dep_id;
                *running_dep_id += 1;
                id
            };

            let is_write = access.1;
            if !rt_check.insert(
                se,
                cx,
                lp,
                ptr,
                ptr_expr,
                access_ty,
                is_write,
                dep_id,
                as_id,
                self.btc,
                self.symbolic_max_btc,
                needs_freeze,
            ) {
                rt_check.pointers.truncate(num_pointers);
                return false;
            }
        }

        true
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1448
    /// `canCheckPtrAtRT`: whether the pointers can be checked at run time for
    /// non-intersection. True if we need no check or if we do and we can
    /// generate them (i.e. the pointers have computable bounds). False means
    /// we couldn't analyze and generate runtime checks for all pointers in the
    /// loop, but if `allow_partial` is set then we will have checks for those
    /// pointers we could analyze. `dep_checker` is used to remove unknown
    /// dependences from `dep_cands`.
    #[allow(clippy::too_many_arguments)]
    pub fn can_check_ptr_at_rt(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        rt_check: &mut RuntimePointerChecking,
        dep_cands: &mut DepCandidates,
        strides: &SymbolicStrideMap,
        uncomputable_ptr: &mut Option<Operand>,
        allow_partial: bool,
        dep_checker: &MemoryDepChecker,
    ) -> bool {
        // Find pointers with computable bounds. We are going to use this
        // information to place a runtime bound check.
        let mut can_do_rt = true;

        let mut may_need_rt_check = false;
        if !self.is_rt_check_analysis_needed {
            return true;
        }

        if let Some(deps) = dep_checker.dependences() {
            // If there are unknown dependences, this means runtime checks are
            // needed to ensure there's no overlap between accesses to the same
            // underlying object. Remove the equivalence classes containing both
            // source and destination accesses from DepCands. This ensures
            // runtime checks will be generated between those accesses and
            // prevents them from being grouped together.
            for dep in deps {
                if dep.ty != DepType::Unknown {
                    debug_assert!(
                        dep.ty.is_safe_for_vectorization() == crate::memdepchecker::VectorizationSafetyStatus::Safe,
                        "Should only skip safe dependences"
                    );
                    continue;
                }
                for inst in [dep_checker.source(dep), dep_checker.destination(dep)] {
                    let pointer = load_store_pointer_operand(cx, inst).expect("a memory access");
                    let writes = matches!(
                        cx.function.instruction(inst).opcode,
                        llrm_mir::opcode::Opcode::Store { .. }
                    );
                    dep_cands.erase_class(&(pointer, writes));
                }
            }
        } else {
            self.check_deps.clear();
            *dep_cands = DepCandidates::default();
        }

        // We assign a consecutive id to access from different alias sets.
        // Accesses between different groups doesn't need to be checked.
        let mut as_id = 0u32;
        for as_pointers in self.alias_set_pointers() {
            let mut num_read_ptr_checks = 0;
            let mut num_write_ptr_checks = 0;
            let mut can_do_alias_set_rt = true;
            as_id += 1;

            // We assign consecutive id to access from different dependence
            // sets. Accesses within the same set don't need a runtime check.
            let mut running_dep_id = 1u32;
            let mut dep_set_id: HashMap<Operand, u32> = HashMap::default();

            let mut retries: Vec<(MemAccessInfo, TypeId)> = Vec::new();

            // First, count how many write and read accesses are in the alias
            // set. Also collect MemAccessInfos for later.
            let mut access_infos: Vec<MemAccessInfo> = Vec::new();
            for &ptr in &as_pointers {
                let is_write = self.accesses.contains(&(ptr, true));
                if is_write {
                    num_write_ptr_checks += 1;
                } else {
                    num_read_ptr_checks += 1;
                }
                access_infos.push((ptr, is_write));
            }

            // We do not need runtime checks for this alias set, if there are no
            // writes or a single write and no reads.
            if num_write_ptr_checks == 0 || (num_write_ptr_checks == 1 && num_read_ptr_checks == 0) {
                debug_assert!(
                    as_pointers.len() <= 1 || as_pointers.iter().all(|&ptr| !dep_cands.contains(&(ptr, true))),
                    "Can only skip updating CanDoRT below, if all entries in AS are reads or there is at most 1 entry"
                );
                continue;
            }

            for access in &access_infos {
                for &access_ty in &self.accesses.types.get(access).cloned().unwrap_or_default() {
                    if !self.create_check_for_access(
                        se,
                        cx,
                        rt_check,
                        dep_cands,
                        *access,
                        access_ty,
                        strides,
                        &mut dep_set_id,
                        &mut running_dep_id,
                        as_id,
                        false,
                    ) {
                        retries.push((*access, access_ty));
                        can_do_alias_set_rt = false;
                    }
                }
            }

            // Note that this function computes CanDoRT and MayNeedRTCheck
            // independently. For example CanDoRT=false, MayNeedRTCheck=false
            // means that we have a pointer for which we couldn't find the
            // bounds but we don't actually need to emit any checks so it does
            // not matter.
            //
            // We need runtime checks for this alias set, if there are at least
            // 2 dependence sets (in which case RunningDepId > 2) or if we need
            // to re-try any bound checks (because in that case the number of
            // dependence sets is incomplete).
            let needs_alias_set_rt_check = running_dep_id > 2 || !retries.is_empty();

            // We need to perform run-time alias checks, but some pointers had
            // bounds that couldn't be checked.
            if needs_alias_set_rt_check && !can_do_alias_set_rt {
                // Reset the CanDoSetRt flag and retry all accesses that have
                // failed. We know that we need these checks, so we can now be
                // more aggressive and add further checks if required (overflow
                // checks).
                can_do_alias_set_rt = true;
                for &(access, access_ty) in &retries {
                    if !self.create_check_for_access(
                        se,
                        cx,
                        rt_check,
                        dep_cands,
                        access,
                        access_ty,
                        strides,
                        &mut dep_set_id,
                        &mut running_dep_id,
                        as_id,
                        true,
                    ) {
                        can_do_alias_set_rt = false;
                        *uncomputable_ptr = Some(access.0);
                        if !allow_partial {
                            break;
                        }
                    }
                }
            }

            can_do_rt &= can_do_alias_set_rt;
            may_need_rt_check |= needs_alias_set_rt_check;
            as_id += 1;
        }

        // If the pointers that we would use for the bounds comparison have
        // different address spaces, assume the values aren't directly
        // comparable, so we can't use them for the runtime check. We also have
        // to assume they could overlap. In the future there should be metadata
        // for whether address spaces are disjoint.
        let num_pointers = rt_check.pointers.len();
        for i in 0..num_pointers {
            for j in i + 1..num_pointers {
                let (one, other) = (&rt_check.pointers[i], &rt_check.pointers[j]);
                // Only need to check pointers between two different dependency
                // sets.
                if one.dependency_set_id == other.dependency_set_id {
                    continue;
                }
                // Only need to check pointers in the same alias set.
                if one.alias_set_id != other.alias_set_id {
                    continue;
                }

                let space_i = crate::loopaccess::address_space(cx, one.pointer_value);
                let space_j = crate::loopaccess::address_space(cx, other.pointer_value);
                if space_i != space_j {
                    return false;
                }
            }
        }

        if may_need_rt_check && (can_do_rt || allow_partial) {
            rt_check.generate_checks(se, cx, dep_checker, dep_cands);
        }

        // If we can do run-time checks, but there are no checks, no runtime
        // checks are needed. This can happen when all pointers point to the
        // same underlying object for example.
        rt_check.need = if can_do_rt { rt_check.number_of_checks() != 0 } else { may_need_rt_check };

        let can_do_rt_if_needed = !rt_check.need || can_do_rt;
        debug_assert!(
            can_do_rt_if_needed == (can_do_rt || !may_need_rt_check),
            "CanDoRTIfNeeded depends on RtCheck.Need"
        );
        if !can_do_rt_if_needed && !allow_partial {
            rt_check.reset();
        }
        can_do_rt_if_needed
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1626
    /// `buildDependenceSets`: goes over all memory accesses, checks whether a
    /// RT check is needed and builds sets of dependent accesses.
    pub fn build_dependence_sets(
        &mut self,
        cx: &Cx,
        dep_cands: &mut DepCandidates,
    ) {
        // We process the set twice: first we process read-write pointers, last
        // we process read-only pointers. This allows us to skip dependence
        // tests for read-only pointers.

        // The AliasSetTracker has nicely partitioned our pointers by metadata
        // compatibility and potential for underlying-object overlap. As a
        // result, we only need to check for potential pointer dependencies
        // within each alias set.
        for as_pointers in self.alias_set_pointers() {
            let mut alias_set_has_write = false;

            // Map of (pointer to underlying objects, accessed address space) to
            // last access encountered.
            let mut obj_to_last_access: HashMap<(Operand, u32), MemAccessInfo> = HashMap::default();

            // Set of access to check after all writes have been processed.
            let mut deferred_accesses = PtrAccessMap::default();

            // Iterate over each alias set twice, once to process read/write
            // pointers, and then to process read-only pointers.
            for use_deferred in [false, true] {
                // Note that both the alias-set tracker and the alias sets
                // themselves used ordered collections internally and so the
                // iteration order here is deterministic.
                for &ptr in &as_pointers {
                    // For a single memory access in AliasSetTracker, Accesses
                    // may contain both read and write, and they both need to
                    // be handled for CheckDeps.
                    let keys: Vec<MemAccessInfo> =
                        if use_deferred { deferred_accesses.order.clone() } else { self.accesses.order.clone() };
                    for (access_ptr, is_write) in keys {
                        if access_ptr != ptr {
                            continue;
                        }

                        // If we're using the deferred access set, then it
                        // contains only reads.
                        let is_read_only_ptr = self.read_only_ptr.contains(&ptr) && !is_write;
                        if use_deferred && !is_read_only_ptr {
                            continue;
                        }

                        let access: MemAccessInfo = (ptr, is_write);
                        dep_cands.insert(&access);

                        // Memorize read-only pointers for later processing and
                        // skip them in the first round (they need to be checked
                        // after we have seen all write pointers). Note: we also
                        // mark pointer that are not consecutive as "read-only"
                        // pointers (so that we check "a[b[i]] +="). Hence, we
                        // need the second check for "!IsWrite".
                        if !use_deferred && is_read_only_ptr {
                            // We only use the pointer keys, the types vector
                            // values don't matter.
                            deferred_accesses.insert(access, None);
                            continue;
                        }

                        // If this is a write - check other reads and writes for
                        // conflicts. If this is a read only check other writes
                        // for conflicts (but only if there is no other write to
                        // the ptr - this is an optimization to catch "a[i] =
                        // a[i] + " without having to do a dependence check).
                        if (is_write || is_read_only_ptr) && alias_set_has_write {
                            self.check_deps.push(access);
                            self.is_rt_check_analysis_needed = true;
                        }

                        if is_write {
                            alias_set_has_write = true;
                        }

                        // Create sets of pointers connected by a shared alias
                        // set and underlying object.
                        let q = cx.query(None);
                        let uos = get_underlying_objects(&q, ptr, Some(cx.loops), MAX_LOOKUP_SEARCH_DEPTH);
                        let space = crate::loopaccess::address_space(cx, ptr).expect("a pointer");
                        for underlying_obj in uos {
                            // nullptr never alias, don't join sets for pointer
                            // that have "null" in their UnderlyingObjects list.
                            if is_null_pointer(cx, underlying_obj) && !null_pointer_is_defined(cx) {
                                continue;
                            }

                            match obj_to_last_access.entry((underlying_obj, space)) {
                                std::collections::hash_map::Entry::Vacant(slot) => {
                                    slot.insert(access);
                                }
                                std::collections::hash_map::Entry::Occupied(mut slot) => {
                                    dep_cands.union_sets(&access, slot.get());
                                    slot.insert(access);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `isa<ConstantPointerNull>`.
fn is_null_pointer(
    cx: &Cx,
    operand: Operand,
) -> bool {
    matches!(
        operand,
        Operand::Constant(id) if matches!(cx.context.get(id).kind, llrm_mir::context::ConstantKind::Null)
    )
}
