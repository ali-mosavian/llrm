//! `LoopAccessInfo`: whether the memory accesses of an innermost loop can be
//! reordered across iterations, and the runtime checks that would make them
//! so.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`
//! (`LoopAccessInfo::canAnalyzeLoop`, `analyzeLoop`, `print`). The loop's
//! loads and stores are collected in reverse postorder (a call or any other
//! memory instruction that is not a simple load or store makes the loop
//! unanalysable); the pointers they use are given to an [`AccessAnalysis`] that
//! partitions them by what they may alias, builds the dependence classes and
//! finds the bounds for runtime checks; the [`MemoryDepChecker`] then
//! classifies the dependences within the classes, and if those are not provably
//! safe but may be with checks, the bounds are found again for every pointer.
//!
//! # Example
//! `a[i + 1] = a[i] + x` over a loop with a known trip count: "Memory
//! dependences are safe", one forward dependence, no checks.
//!
//! # DEVIATE 24 (no predicated SCEV), 25 (no symbolic strides)
//! See [`crate::loopaccess`]: the counts are the loop's own, no predicate is
//! ever collected, and `collectStridedAccess` finds nothing to speculate.
//!
//! # DEVIATE 27 (no loop metadata, no debug locations)
//! MIR has no `llvm.loop.parallel_accesses` annotation
//! (`isAnnotatedParallel` is false), no `llvm.loop.distribute.enable` and no
//! debug location on a remark ("Memory location is the same as accessed at"
//! is not added), and no vector function database (`VFDatabase`).

use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::loopinfo::LoopGraph;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::valuetracking::guarantee::{may_read_from_memory, may_write_to_memory};
use llrm_support::hash::HashSet;

use crate::aa::{AAInfo, AAResults, LocationSize, MemoryLocation};
use crate::accessanalysis::AccessAnalysis;
use crate::loopaccess::{
    SymbolicStrideMap, block_needs_predication, get_ptr_stride, load_store_pointer_operand, load_store_type,
    visit_pointers,
};
use crate::memdepchecker::{DepType, MemoryDepChecker};
use crate::runtimecheck::{DepCandidates, RuntimePointerChecking, VectorizerParams};
use crate::scev::backedge::ExitCountKind;
use crate::scev::{Cx, ScalarEvolution};

/// `LoopAccessInfo`.
pub struct LoopAccessInfo {
    /// We need to check that all of the pointers in this list are disjoint at
    /// runtime.
    pt_rt_checking: RuntimePointerChecking,
    /// The dependence checker.
    dep_checker: MemoryDepChecker,
    the_loop: BlockId,
    /// Number of memory accesses in the loop.
    num_loads: u32,
    num_stores: u32,
    /// Whether the analysis found the loop safe to vectorize memory-wise.
    can_vec_mem: bool,
    /// Whether a runtime check cannot be inserted because of a convergent
    /// operation.
    has_convergent_op: bool,
    /// Whether we have a store-load or store-store dependence through
    /// loop-invariant addresses.
    has_load_store_dependence_involving_loop_invariant_address: bool,
    has_store_store_dependence_involving_loop_invariant_address: bool,
    /// The stores to loop-invariant addresses.
    stores_to_invariant_addresses: Vec<InstId>,
    /// Whether `ptr_rt_checking` has checks for all the pointers (or partial
    /// ones, if `allow_partial`).
    has_complete_ptr_rt_checking: bool,
    allow_partial: bool,
    symbolic_strides: SymbolicStrideMap,
    /// The remark of why the loop cannot be analysed (`Report->getMsg()`).
    report: Option<String>,
}

impl LoopAccessInfo {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L3285
    /// `LoopAccessInfo(L, SE, TTI, TLI, AA, DT, LI, AC, AllowPartial)`.
    /// `max_target_vector_width_in_bits` is twice the target's fixed vector
    /// register width (`u32::MAX` where there is no cost model).
    pub fn new<A: AAResults>(
        se: &mut ScalarEvolution,
        cx: &Cx,
        the_loop: BlockId,
        aa: &mut A,
        allow_partial: bool,
        max_target_vector_width_in_bits: u32,
    ) -> Self {
        let btc = se.get_backedge_taken_count(cx, the_loop, ExitCountKind::Exact);
        let symbolic_max_btc = se.get_backedge_taken_count(cx, the_loop, ExitCountKind::SymbolicMaximum);
        let mut info = Self {
            pt_rt_checking: RuntimePointerChecking::new(VectorizerParams::default()),
            dep_checker: MemoryDepChecker::new(the_loop, max_target_vector_width_in_bits, btc, symbolic_max_btc),
            the_loop,
            num_loads: 0,
            num_stores: 0,
            can_vec_mem: false,
            has_convergent_op: false,
            has_load_store_dependence_involving_loop_invariant_address: false,
            has_store_store_dependence_involving_loop_invariant_address: false,
            stores_to_invariant_addresses: Vec::new(),
            has_complete_ptr_rt_checking: false,
            allow_partial,
            symbolic_strides: SymbolicStrideMap::default(),
            report: None,
        };
        if info.can_analyze_loop(se, cx) {
            info.can_vec_mem = info.analyze_loop(se, cx, aa);
        }
        info
    }

    /// `getRuntimePointerChecking`.
    pub fn runtime_pointer_checking(&self) -> &RuntimePointerChecking {
        &self.pt_rt_checking
    }

    /// `getDepChecker`.
    pub fn dep_checker(&self) -> &MemoryDepChecker {
        &self.dep_checker
    }

    /// `canVectorizeMemory`: return true we can analyze the memory accesses in
    /// the loop and there are no memory dependence cycles. Note that for
    /// dependences between loads and stores with uniform addresses,
    /// `hasStoreStoreDependenceInvolvingLoopInvariantAddress` and
    /// `hasLoadStoreDependenceInvolvingLoopInvariantAddress` also need to be
    /// checked.
    pub fn can_vectorize_memory(&self) -> bool {
        self.can_vec_mem
    }

    /// `hasConvergentOp`.
    pub fn has_convergent_op(&self) -> bool {
        self.has_convergent_op
    }

    /// `getNumLoads`.
    pub fn num_loads(&self) -> u32 {
        self.num_loads
    }

    /// `getNumStores`.
    pub fn num_stores(&self) -> u32 {
        self.num_stores
    }

    /// `getStoresToInvariantAddresses`.
    pub fn stores_to_invariant_addresses(&self) -> &[InstId] {
        &self.stores_to_invariant_addresses
    }

    /// `hasLoadStoreDependenceInvolvingLoopInvariantAddress`.
    pub fn has_load_store_dependence_involving_loop_invariant_address(&self) -> bool {
        self.has_load_store_dependence_involving_loop_invariant_address
    }

    /// `hasStoreStoreDependenceInvolvingLoopInvariantAddress`.
    pub fn has_store_store_dependence_involving_loop_invariant_address(&self) -> bool {
        self.has_store_store_dependence_involving_loop_invariant_address
    }

    /// `hasAllowPartial`.
    pub fn has_allow_partial(&self) -> bool {
        self.allow_partial
    }

    /// `getReport`: why the loop was not analysed.
    pub fn report(&self) -> Option<&str> {
        self.report.as_deref()
    }

    /// `recordAnalysis`: the first reason wins (the C++ asserts there is only
    /// one).
    fn record_analysis(
        &mut self,
        message: impl Into<String>,
    ) {
        debug_assert!(self.report.is_none(), "Multiple reports generated");
        self.report = Some(message.into());
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2659
    /// `canAnalyzeLoop`.
    fn can_analyze_loop(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
    ) -> bool {
        let the_loop = cx.loops.get_loop_for(self.the_loop).expect("the header of a loop");
        // We can only analyze innermost loops.
        if !the_loop.is_innermost() {
            self.record_analysis("loop is not the innermost loop");
            return false;
        }

        // We must have a single backedge.
        if the_loop.get_num_back_edges(cx.function) != 1 {
            self.record_analysis("loop control flow is not understood by analyzer");
            return false;
        }

        // ScalarEvolution needs to be able to find the symbolic max backedge
        // taken count, which is an upper bound on the number of loop
        // iterations. The loop may execute fewer iterations, if it exits via
        // an uncountable exit.
        let exit_count = se.get_backedge_taken_count(cx, self.the_loop, ExitCountKind::SymbolicMaximum);
        if se.is_could_not_compute(exit_count) {
            self.record_analysis("could not determine number of loop iterations");
            return false;
        }

        true
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2697
    /// `analyzeLoop`.
    fn analyze_loop<A: AAResults>(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        aa: &mut A,
    ) -> bool {
        let lp = self.the_loop;
        let q = cx.query(None);

        // Holds the Load and Store instructions.
        let mut loads: Vec<InstId> = Vec::new();
        let mut stores: Vec<InstId> = Vec::new();

        // Holds all the different accesses in the loop.
        let mut num_reads = 0u32;
        let mut num_read_writes = 0u32;

        let mut has_complex_mem_inst = false;

        // A runtime check is only legal to insert if there are no convergent
        // calls.
        self.has_convergent_op = false;

        self.pt_rt_checking.pointers.clear();
        self.pt_rt_checking.need = false;

        // `isAnnotatedParallel` is false (D27).
        let is_annotated_parallel = false;

        // `EnableMemAccessVersioning` records nothing (D25).

        // Traverse blocks in fixed RPOT order, regardless of their storage in
        // the loop info, as it may be arbitrary.
        let rpot = loop_blocks_rpo(cx, lp);

        // Don't return early as soon as we found a memory access that cannot
        // be vectorized - HasConvergentOp must still be computed as it is part
        // of LAI's public API (used by LoopDistribute).
        for &bb in &rpot {
            // Scan the BB and collect legal loads and stores. Also detect any
            // convergent instructions.
            for inst in cx.function.iter_block(bb) {
                let instruction = cx.function.instruction(inst);
                let is_call = matches!(instruction.opcode, Opcode::Call(_) | Opcode::Invoke(_));
                if is_call && call_is_convergent(cx, inst) {
                    self.has_convergent_op = true;
                }

                // Unsafe to vectorize and we already found a convergent
                // operation, can early return now.
                if has_complex_mem_inst && self.has_convergent_op {
                    return false;
                }

                // Already unsafe to vectorize; keep scanning for convergent
                // ops.
                if has_complex_mem_inst {
                    continue;
                }

                // Many math library functions read the rounding mode. We will
                // only vectorize a loop if it contains known function calls
                // that don't set the flag. Therefore, it is safe to ignore
                // this read from memory.
                if matches!(instruction.opcode, Opcode::Call(_)) && is_vector_intrinsic_call(cx, inst) {
                    continue;
                }

                // If this is a load, save it. If this instruction can read
                // from memory but is not a load, we only allow it if it's a
                // call to a function with a vector mapping and no pointer
                // arguments (there are no vector mappings here).
                if may_read_from_memory(&q, inst) {
                    let Opcode::Load { volatile, .. } = instruction.opcode else {
                        self.record_analysis("instruction cannot be vectorized");
                        has_complex_mem_inst = true;
                        continue;
                    };
                    if volatile && !is_annotated_parallel {
                        self.record_analysis("read with atomic ordering or volatile read");
                        has_complex_mem_inst = true;
                        continue;
                    }
                    self.num_loads += 1;
                    loads.push(inst);
                    self.dep_checker.add_access(cx, inst);
                    continue;
                }

                // Save 'store' instructions. Abort if other instructions write
                // to memory.
                if may_write_to_memory(&q, inst) {
                    let Opcode::Store { volatile, .. } = instruction.opcode else {
                        self.record_analysis("instruction cannot be vectorized");
                        has_complex_mem_inst = true;
                        continue;
                    };
                    if volatile && !is_annotated_parallel {
                        self.record_analysis("write with atomic ordering or volatile write");
                        has_complex_mem_inst = true;
                        continue;
                    }
                    self.num_stores += 1;
                    stores.push(inst);
                    self.dep_checker.add_access(cx, inst);
                }
            } // Next instr.
        } // Next block.

        if has_complex_mem_inst {
            return false;
        }

        // Now we have two lists that hold the loads and the stores. Next, we
        // find the pointers that they use.

        // Check if we see any stores. If there are no stores, then we don't
        // care if the pointers are *restrict*.
        if stores.is_empty() {
            return true;
        }

        let mut dep_cands = DepCandidates::default();
        let btc = se.get_backedge_taken_count(cx, lp, ExitCountKind::Exact);
        let symbolic_max_btc = se.get_backedge_taken_count(cx, lp, ExitCountKind::SymbolicMaximum);
        let mut accesses = AccessAnalysis::new(lp, aa, btc, symbolic_max_btc);

        // Holds the analyzed pointers. We don't want to call
        // getUnderlyingObjects multiple times on the same object. If the ptr is
        // accessed twice, once for read and once for write, it will only appear
        // once (on the write list). This is okay, since we are going to check
        // for conflicts between writes and between reads and writes, but not
        // between reads and reads.
        let mut seen: HashSet<(Operand, llrm_mir::types::TypeId)> = HashSet::default();

        // Record uniform store addresses to identify if we have multiple
        // stores to the same address.
        let mut uniform_stores: HashSet<Operand> = HashSet::default();

        for &st in &stores {
            let ptr = load_store_pointer_operand(cx, st).expect("a store");

            if self.is_invariant(se, cx, ptr) {
                // Record store instructions to loop invariant addresses
                self.stores_to_invariant_addresses.push(st);
                self.has_store_store_dependence_involving_loop_invariant_address |= !uniform_stores.insert(ptr);
            }

            // If we did *not* see this pointer before, insert it to the
            // read-write list. At this phase it is only a 'write' list.
            let access_ty = load_store_type(cx, st).expect("a store");
            if seen.insert((ptr, access_ty)) {
                num_read_writes += 1;

                let mut loc = memory_location(cx, st);
                // The TBAA metadata could have a control dependency on the
                // predication condition, so we cannot rely on it when
                // determining whether or not we need runtime pointer checks.
                if block_needs_predication(cx, cx.function.parent(st).expect("a block"), lp) {
                    loc.aa_info.tbaa = None;
                }

                // Expand forked pointers (i.e., a phi of multiple strided
                // pointers) into all alternatives.
                visit_pointers(cx, loc.ptr, lp, |ptr| {
                    accesses.add_store(MemoryLocation { ptr, ..loc }, access_ty);
                });
            }
        }

        if is_annotated_parallel {
            return true;
        }

        for &ld in &loads {
            let ptr = load_store_pointer_operand(cx, ld).expect("a load");
            // If we did *not* see this pointer before, insert it to the read
            // list. If we *did* see it before, then it is already in the
            // read-write list. This allows us to vectorize expressions such as
            // A[i] += x; Because the address of A[i] is a read-write pointer.
            // This only works if the index of A[i] is strictly monotonic, which
            // we approximate (conservatively) via getPtrStride. If the address
            // is unknown (e.g. A[B[i]]) then we may read, modify, and write
            // overlapping words. Note that "zero stride" is unsafe and is being
            // handled below.
            let mut is_read_only_ptr = false;
            let access_ty = load_store_type(cx, ld).expect("a load");
            if seen.insert((ptr, access_ty))
                || get_ptr_stride(se, cx, access_ty, ptr, lp, &self.symbolic_strides, true).is_none()
            {
                num_reads += 1;
                is_read_only_ptr = true;
            }

            // See if there is an unsafe dependency between a load to a uniform
            // address and store to the same uniform address.
            if uniform_stores.contains(&ptr) {
                self.has_load_store_dependence_involving_loop_invariant_address = true;
            }

            let mut loc = memory_location(cx, ld);
            // The TBAA metadata could have a control dependency on the
            // predication condition, so we cannot rely on it when determining
            // whether or not we need runtime pointer checks.
            if block_needs_predication(cx, cx.function.parent(ld).expect("a block"), lp) {
                loc.aa_info.tbaa = None;
            }

            // Expand forked pointers (i.e., a phi of multiple strided pointers)
            // into all alternatives.
            visit_pointers(cx, loc.ptr, lp, |ptr| {
                accesses.add_load(MemoryLocation { ptr, ..loc }, access_ty, is_read_only_ptr);
            });
        }

        // If we write (or read-write) to a single destination and there are no
        // other reads in this loop then is it safe to vectorize: the vectorized
        // stores preserve ordering via replication or order-preserving
        // @llvm.masked.scatter.
        if num_read_writes == 1 && num_reads == 0 {
            return true;
        }

        // Build dependence sets and check whether we need a runtime pointer
        // bounds check.
        accesses.build_dependence_sets(cx, &mut dep_cands);

        // Find pointers with computable bounds. We are going to use this
        // information to place a runtime bound check.
        let mut uncomputable_ptr = None;
        self.has_complete_ptr_rt_checking = accesses.can_check_ptr_at_rt(
            se,
            cx,
            &mut self.pt_rt_checking,
            &mut dep_cands,
            &self.symbolic_strides,
            &mut uncomputable_ptr,
            self.allow_partial,
            &self.dep_checker,
        );
        if !self.has_complete_ptr_rt_checking {
            self.record_analysis("cannot identify array bounds");
            return false;
        }

        let mut deps_are_safe = true;
        if accesses.is_dependency_check_needed() {
            let mut caches = self.pt_rt_checking.caches();
            deps_are_safe =
                self.dep_checker.are_deps_safe(se, cx, &mut caches, &dep_cands, accesses.dependencies_to_check());

            if !deps_are_safe && self.dep_checker.should_retry_with_runtime_checks() {
                self.pt_rt_checking.reset();
                self.pt_rt_checking.need = true;

                uncomputable_ptr = None;
                self.has_complete_ptr_rt_checking = accesses.can_check_ptr_at_rt(
                    se,
                    cx,
                    &mut self.pt_rt_checking,
                    &mut dep_cands,
                    &self.symbolic_strides,
                    &mut uncomputable_ptr,
                    self.allow_partial,
                    &self.dep_checker,
                );

                // Check that we found the bounds for the pointer.
                if !self.has_complete_ptr_rt_checking {
                    self.record_analysis("cannot check memory dependencies at runtime");
                    return false;
                }

                // Clear the dependency checks. They are no longer needed.
                accesses.reset_dep_checks(&mut self.dep_checker);

                deps_are_safe = true;
            }
        }

        // Update the invariant address dependence flags based on dependences
        // found by the dep checker. Even if dependences were not recorded (too
        // many to track), any InvariantUnsafe dep would still have set the
        // status to Unsafe
        if let Some(deps) = self.dep_checker.dependences() {
            for dep in deps {
                if dep.ty != DepType::InvariantUnsafe {
                    continue;
                }
                let src = self.dep_checker.source(dep);
                let dst = self.dep_checker.destination(dep);
                let is_load = |inst: InstId| matches!(cx.function.instruction(inst).opcode, Opcode::Load { .. });
                if is_load(src) != is_load(dst) {
                    self.has_load_store_dependence_involving_loop_invariant_address = true;
                } else {
                    debug_assert!(
                        matches!(cx.function.instruction(src).opcode, Opcode::Store { .. })
                            && matches!(cx.function.instruction(dst).opcode, Opcode::Store { .. }),
                        "Expected both to be stores"
                    );
                    self.has_store_store_dependence_involving_loop_invariant_address = true;
                }
            }
        }

        if self.has_convergent_op {
            self.record_analysis("cannot add control dependency to convergent operation");
            return false;
        }

        if deps_are_safe {
            return true;
        }

        self.emit_unsafe_dependence_remark();
        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L3038
    /// `emitUnsafeDependenceRemark`: the report of the first unsafe
    /// dependence.
    fn emit_unsafe_dependence_remark(&mut self) {
        let Some(deps) = self.dep_checker.dependences() else { return };
        let Some(found) = deps
            .iter()
            .find(|dep| dep.ty.is_safe_for_vectorization() != crate::memdepchecker::VectorizationSafetyStatus::Safe)
        else {
            return;
        };

        // `llvm.loop.distribute.enable` is not there (D27).
        let mut info = String::from(
            "unsafe dependent memory operations in loop. Use #pragma clang loop distribute(enable) to allow loop \
             distribution to attempt to isolate the offending operations into a separate loop",
        );
        info.push_str(match found.ty {
            DepType::NoDep | DepType::Forward | DepType::BackwardVectorizable => unreachable!("Unexpected dependence"),
            DepType::Backward => "\nBackward loop carried data dependence.",
            DepType::ForwardButPreventsForwarding => {
                "\nForward loop carried data dependence that prevents store-to-load forwarding."
            }
            DepType::BackwardVectorizableButPreventsForwarding => {
                "\nBackward loop carried data dependence that prevents store-to-load forwarding."
            }
            DepType::IndirectUnsafe => "\nUnsafe indirect dependence.",
            DepType::InvariantUnsafe => "\nUnsafe dependence on loop-invariant address.",
            DepType::Unknown => "\nUnknown data dependence.",
        });
        self.record_analysis(info);
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L3135
    /// `isInvariant`: whether `value` is loop-invariant, in the IR or by SCEV.
    pub fn is_invariant(
        &self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        value: Operand,
    ) -> bool {
        let found = cx.loops.get_loop_for(self.the_loop).expect("the header of a loop");
        if found.is_loop_invariant(cx.function, value) {
            return true;
        }
        if !se.is_value_scevable(cx, value) {
            return false;
        }
        let s = se.get_scev(cx, value);
        se.is_loop_invariant(cx, s, Some(self.the_loop))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L3307
    /// `print`: `opt -passes='print<access-info>'` lists a loop so. `text`
    /// writes a value as `opt` does.
    pub fn print(
        &self,
        se: &ScalarEvolution,
        cx: &Cx,
        labels: &crate::scev::print::Labels,
        text: &dyn crate::runtimecheck::ValueText,
        depth: usize,
    ) -> String {
        let indent = " ".repeat(depth);
        let mut out = String::new();
        if self.can_vec_mem {
            out += &format!("{indent}Memory dependences are safe");
            let dc = &self.dep_checker;
            if !dc.is_safe_for_any_vector_width() {
                out += &format!(" with a maximum safe vector width of {} bits", dc.max_safe_vector_width_in_bits());
            }
            if !dc.is_safe_for_any_store_load_forward_distances() {
                out += &format!(
                    ", with a maximum safe store-load forward width of {} bits",
                    dc.store_load_forward_safe_distance_in_bits()
                );
            }
            if self.pt_rt_checking.need {
                out += " with run-time checks";
            }
            out += "\n";
        }

        if self.has_convergent_op {
            out += &format!("{indent}Has convergent operation in loop\n");
        }

        if let Some(report) = &self.report {
            out += &format!("{indent}Report: {report}\n");
        }

        if let Some(dependences) = self.dep_checker.dependences() {
            out += &format!("{indent}Dependences:\n");
            for dep in dependences {
                out += &dep.print(depth + 2, self.dep_checker.memory_instructions(), text);
                out += "\n";
            }
        } else {
            out += &format!("{indent}Too many dependences, not recorded\n");
        }

        // List the pair of accesses need run-time checks to prove
        // independence.
        out += &self.pt_rt_checking.print(se, cx, labels, text, depth);
        if self.pt_rt_checking.need && !self.has_complete_ptr_rt_checking {
            out += &format!("{indent}Generated run-time checks are incomplete\n");
        }
        out += "\n";

        let found = self.has_store_store_dependence_involving_loop_invariant_address
            || self.has_load_store_dependence_involving_loop_invariant_address;
        out += &format!(
            "{indent}Non vectorizable stores to invariant address were {}found in loop.\n",
            if found { "" } else { "not " }
        );

        // No predicate is ever collected (D24).
        out += &format!("{indent}SCEV assumptions:\n");
        out += "\n";
        out += &format!("{indent}Expressions re-written:\n");
        out
    }
}

/// `MemoryLocation::get(Instruction)` of a load or a store.
fn memory_location(
    cx: &Cx,
    inst: InstId,
) -> MemoryLocation {
    let instruction = cx.function.instruction(inst);
    let ptr = load_store_pointer_operand(cx, inst).expect("a load or a store");
    let ty = load_store_type(cx, inst).expect("a load or a store");
    let bytes = cx.layout.store_size(&cx.context.types, ty);
    MemoryLocation { ptr, size: LocationSize::Precise(bytes as u64), aa_info: AAInfo::of(instruction) }
}

/// `LoopBlocksRPO`: the blocks of the loop from its header, reverse postorder.
fn loop_blocks_rpo(
    cx: &Cx,
    header: BlockId,
) -> Vec<BlockId> {
    let found = cx.loops.get_loop_for(header).expect("the header of a loop");
    let mut visited = llrm_mir::dense::IdSet::<BlockId>::new();
    let mut post: Vec<BlockId> = Vec::new();
    // The blocks to finish, each with the successors left to visit.
    let mut stack: Vec<(BlockId, Vec<BlockId>)> = Vec::new();
    visited.insert(header);
    stack.push((header, cx.function.children(header).collect()));
    while let Some((block, successors)) = stack.last_mut() {
        match successors.pop() {
            Some(next) => {
                if found.contains(next) && visited.insert(next) {
                    let children: Vec<BlockId> = cx.function.children(next).collect();
                    stack.push((next, children));
                }
            }
            None => {
                post.push(*block);
                stack.pop();
            }
        }
    }
    post.reverse();
    post
}

/// Whether the call `inst` is `convergent`, itself or by its callee.
fn call_is_convergent(
    cx: &Cx,
    inst: InstId,
) -> bool {
    let q = cx.query(None);
    let instruction = cx.function.instruction(inst);
    let flag =
        |attrs: &[Attribute]| attrs.iter().any(|one| matches!(one, Attribute::Flag(name) if name == "convergent"));
    llrm_mir::valuetracking::pattern::call_info(instruction).is_some_and(|info| flag(&info.attrs))
        || llrm_mir::valuetracking::pattern::called_function(&q, instruction).is_some_and(|callee| flag(&callee.attrs))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/VectorUtils.cpp#L234
/// `getVectorIntrinsicIDForCall(Call, TLI) != not_intrinsic`: a call of an
/// intrinsic that is trivially vectorizable, or `lifetime.start`/`end` or
/// `assume`.
fn is_vector_intrinsic_call(
    cx: &Cx,
    inst: InstId,
) -> bool {
    matches!(
        cx.query(None).intrinsic(inst),
        Some(
            Intrinsic::WithOverflow { .. }
                | Intrinsic::Saturating { .. }
                | Intrinsic::MinMax { .. }
                | Intrinsic::Fixed { .. }
                | Intrinsic::BitCount(_)
                | Intrinsic::ByteSwap
                | Intrinsic::BitReverse
                | Intrinsic::FunnelShift { .. }
                | Intrinsic::Abs
                | Intrinsic::Compare { .. }
                | Intrinsic::FMulAdd
                | Intrinsic::CopySign
                | Intrinsic::Unary(_)
                | Intrinsic::LRint
                | Intrinsic::LLRint
                | Intrinsic::LifetimeStart
                | Intrinsic::LifetimeEnd
                | Intrinsic::Assume
        )
    )
}

#[cfg(test)]
#[path = "loopaccessinfo_tests.rs"]
mod tests;
