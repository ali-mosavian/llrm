//! `MemoryDepChecker`: whether the accesses of a loop to memory that may be
//! the same can be reordered across iterations, and by how much.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`. Each load
//! and store of the loop is numbered in program order (`addAccess`; a phi of
//! pointers inside the loop gives one access for each incoming pointer).
//! `areDepsSafe` takes each dependence class of the accesses that need a check
//! and classifies every pair, a write against another access, by `isDependent`:
//! the SCEV distance between the two pointers, in bytes, against the strides
//! and the sizes of the accesses and the loop's trip count. The result is the
//! worst status over the pairs, the smallest dependence distance, and the
//! dependences themselves while there are fewer than `-max-dependences`.
//!
//! # Example
//! `a[i + 1] = a[i]` is a forward dependence of distance one element: the
//! load is safe to run ahead of the store. `a[i] = a[i + 1]` is a backward
//! dependence, safe to vectorize when the distance covers the vector.
//!
//! # DEVIATE 24 (no predicated SCEV)
//! Where LLVM adds a wrap predicate or an addrec-with-predicates to prove a
//! stride (`getPtrStride` with a `Predicates` argument) the access has no
//! stride here and the pair is `IndirectUnsafe`.

use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::IntPredicate;
use llrm_support::hash::{HashMap, HashSet};

use crate::loopaccess::{
    SymbolicStrideMap, get_ptr_stride, load_store_pointer_operand, load_store_type, visit_pointers,
};
use crate::runtimecheck::{AccessOrder, DepCandidates, MemAccessInfo, PointerBounds, get_start_and_end_for_access};
use crate::scev::expr::{Scev, Use};
use crate::scev::guards::LoopGuards;
use crate::scev::{Cx, ScalarEvolution};

/// `-max-dependences`: the most dependences recorded.
const MAX_DEPENDENCES: usize = 100;

/// `-store-to-load-forwarding-conflict-detection`.
const ENABLE_FORWARDING_CONFLICT_DETECTION: bool = true;

/// `VectorizerParams::MaxVectorWidth`.
const MAX_VECTOR_WIDTH: u64 = 64;

/// `VectorizerParams::VectorizationFactor` and `VectorizationInterleave`: not
/// forced.
const FORCED_FACTOR: u32 = 0;
const FORCED_INTERLEAVE: u32 = 0;

/// `MemoryDepChecker::VectorizationSafetyStatus`, from most to least
/// permissive.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VectorizationSafetyStatus {
    /// Can vectorize safely without RT checks. All dependences are known to
    /// be safe.
    Safe,
    /// Can possibly vectorize with RT checks to overcome unknown
    /// dependencies.
    PossiblySafeWithRtChecks,
    /// Cannot vectorize due to known unsafe dependencies.
    Unsafe,
}

/// `MemoryDepChecker::Dependence::DepType`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DepType {
    /// No dependence.
    NoDep,
    /// We couldn't determine the direction or the distance.
    Unknown,
    /// At least one of the accesses may access a loop-varying object, like
    /// `A[B[i]]`: no direction or distance, and no runtime check either.
    IndirectUnsafe,
    /// Both accesses to the same loop-invariant address and at least one is a
    /// write.
    InvariantUnsafe,
    /// Lexically forward.
    Forward,
    /// Forward, but if vectorized, is likely to prevent store-to-load
    /// forwarding.
    ForwardButPreventsForwarding,
    /// Lexically backward.
    Backward,
    /// Backward, but the distance allows a vectorization factor dependent on
    /// `MinDepDistBytes`.
    BackwardVectorizable,
    /// Same, but may prevent store-to-load forwarding.
    BackwardVectorizableButPreventsForwarding,
}

impl DepType {
    /// `Dependence::DepName`.
    pub fn name(self) -> &'static str {
        match self {
            DepType::NoDep => "NoDep",
            DepType::Unknown => "Unknown",
            DepType::IndirectUnsafe => "IndirectUnsafe",
            DepType::InvariantUnsafe => "InvariantUnsafe",
            DepType::Forward => "Forward",
            DepType::ForwardButPreventsForwarding => "ForwardButPreventsForwarding",
            DepType::Backward => "Backward",
            DepType::BackwardVectorizable => "BackwardVectorizable",
            DepType::BackwardVectorizableButPreventsForwarding => "BackwardVectorizableButPreventsForwarding",
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1948
    /// `Dependence::isSafeForVectorization`.
    pub fn is_safe_for_vectorization(self) -> VectorizationSafetyStatus {
        match self {
            DepType::NoDep | DepType::Forward | DepType::BackwardVectorizable => VectorizationSafetyStatus::Safe,
            DepType::Unknown => VectorizationSafetyStatus::PossiblySafeWithRtChecks,
            DepType::ForwardButPreventsForwarding
            | DepType::Backward
            | DepType::BackwardVectorizableButPreventsForwarding
            | DepType::IndirectUnsafe
            | DepType::InvariantUnsafe => VectorizationSafetyStatus::Unsafe,
        }
    }
}

/// `MemoryDepChecker::Dependence`: between memory access instructions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Dependence {
    /// Index of the source of the dependence in the instruction map.
    pub source: u32,
    /// Index of the destination of the dependence in the instruction map.
    pub destination: u32,
    pub ty: DepType,
}

impl Dependence {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1967
    /// `isBackward`: lexically backward.
    pub fn is_backward(&self) -> bool {
        matches!(
            self.ty,
            DepType::BackwardVectorizable | DepType::Backward | DepType::BackwardVectorizableButPreventsForwarding
        )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1985
    /// `isPossiblyBackward`: may be a lexically backward dependence type
    /// (includes `Unknown`).
    pub fn is_possibly_backward(&self) -> bool {
        self.is_backward() || matches!(
            self.ty,
            DepType::Unknown | DepType::IndirectUnsafe | DepType::InvariantUnsafe
        )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1990
    /// `isForward`: lexically forward.
    pub fn is_forward(&self) -> bool {
        matches!(
            self.ty,
            DepType::Forward | DepType::ForwardButPreventsForwarding
        )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2651
    /// `print`: `text` writes an instruction as `opt` does.
    pub fn print(
        &self,
        depth: usize,
        instructions: &[InstId],
        text: &dyn crate::runtimecheck::ValueText,
    ) -> String {
        let indent = " ".repeat(depth);
        format!(
            "{indent}{}:\n{indent}  {} -> \n{indent}  {}\n",
            self.ty.name(),
            text.instruction(instructions[self.source as usize]),
            text.instruction(instructions[self.destination as usize])
        )
    }
}

/// `MemoryDepChecker::DepDistanceStrideAndSizeInfo`.
#[derive(Clone, Copy, Debug)]
struct DepDistanceStrideAndSizeInfo {
    dist: Scev,
    max_stride: u64,
    common_stride: Option<u64>,
    type_byte_size: u64,
    a_is_write: bool,
    b_is_write: bool,
}

/// `MemoryDepChecker`.
#[derive(Clone, Debug)]
pub struct MemoryDepChecker {
    /// The header of the innermost loop.
    innermost_loop: BlockId,
    /// Maps access locations (ptr, read/write) to program order.
    accesses: HashMap<MemAccessInfo, Vec<u32>>,
    /// Memory access instructions in program order.
    inst_map: Vec<InstId>,
    /// The program order index to be used for the next instruction.
    access_idx: u32,
    /// The smallest dependence distance in bytes in the loop. This may not be
    /// the same as the maximum number of bytes that are safe to operate on
    /// simultaneously.
    min_dep_dist_bytes: u64,
    /// Number of elements (from consecutive iterations) that are safe to
    /// operate on simultaneously, multiplied by the size of the element in
    /// bits. The size of the element is taken from the memory access that is
    /// most restrictive.
    max_safe_vector_width_in_bits: u64,
    /// Maximum power-of-2 number of elements, which do not prevent store-load
    /// forwarding, multiplied by the size of the elements in bits.
    max_store_load_forward_safe_distance_in_bits: u64,
    /// Whether we should try to vectorize the loop with runtime checks, if
    /// the dependencies are not safe.
    should_retry_with_runtime_checks: bool,
    /// Result of the dependence checks.
    status: VectorizationSafetyStatus,
    /// True if `dependences` reflects the dependences in the loop. If false
    /// we exceeded `MAX_DEPENDENCES` and `dependences` is invalid.
    record_dependences: bool,
    /// Memory dependences collected during the analysis.
    dependences: Vec<Dependence>,
    /// The maximum width of a target's vector registers multiplied by 2 to
    /// also roughly account for additional interleaving.
    max_target_vector_width_in_bits: u32,
    /// The loop's backedge-taken count and symbolic maximum (D24: not through
    /// a `PredicatedScalarEvolution`).
    btc: Scev,
    symbolic_max_btc: Scev,
}

/// The caches the dependence checker and the runtime pointer checking share
/// (`getPointerBounds()` and the `LoopGuards` reference).
pub struct SharedCaches<'a> {
    pub pointer_bounds: &'a mut PointerBounds,
    pub loop_guards: &'a mut Option<LoopGuards>,
}

impl MemoryDepChecker {
    /// `MemoryDepChecker(PSE, AC, DT, L, SymbolicStrides, MaxTargetVectorWidth,
    /// LoopGuards)`; `btc` and `symbolic_max_btc` are `PSE`'s counts.
    pub fn new(
        innermost_loop: BlockId,
        max_target_vector_width_in_bits: u32,
        btc: Scev,
        symbolic_max_btc: Scev,
    ) -> Self {
        Self {
            innermost_loop,
            accesses: HashMap::default(),
            inst_map: Vec::new(),
            access_idx: 0,
            min_dep_dist_bytes: 0,
            max_safe_vector_width_in_bits: u64::from(u32::MAX),
            max_store_load_forward_safe_distance_in_bits: u64::MAX,
            should_retry_with_runtime_checks: false,
            status: VectorizationSafetyStatus::Safe,
            record_dependences: true,
            dependences: Vec::new(),
            max_target_vector_width_in_bits,
            btc,
            symbolic_max_btc,
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1929
    /// `addAccess(StoreInst *)` and `addAccess(LoadInst *)`: the access of
    /// `inst`, once for each pointer a phi of pointers in the loop gives.
    pub fn add_access(
        &mut self,
        cx: &Cx,
        inst: InstId,
    ) {
        let is_write = match cx.function.instruction(inst).opcode {
            llrm_mir::opcode::Opcode::Store { .. } => true,
            llrm_mir::opcode::Opcode::Load { .. } => false,
            _ => return,
        };
        let Some(pointer) = load_store_pointer_operand(cx, inst) else { return };
        visit_pointers(cx, pointer, self.innermost_loop, |ptr| {
            self.accesses.entry((ptr, is_write)).or_default().push(self.access_idx);
            self.inst_map.push(inst);
            self.access_idx += 1;
        });
    }

    /// `isSafeForVectorization`: no memory dependence was encountered that
    /// would inhibit vectorization.
    pub fn is_safe_for_vectorization(&self) -> bool {
        self.status == VectorizationSafetyStatus::Safe
    }

    /// `isSafeForAnyVectorWidth`.
    pub fn is_safe_for_any_vector_width(&self) -> bool {
        self.max_safe_vector_width_in_bits == u64::from(u32::MAX)
    }

    /// `getMaxSafeVectorWidthInBits`.
    pub fn max_safe_vector_width_in_bits(&self) -> u64 {
        self.max_safe_vector_width_in_bits
    }

    /// `isSafeForAnyStoreLoadForwardDistances`.
    pub fn is_safe_for_any_store_load_forward_distances(&self) -> bool {
        self.max_store_load_forward_safe_distance_in_bits == u64::MAX
    }

    /// `getStoreLoadForwardSafeDistanceInBits`.
    pub fn store_load_forward_safe_distance_in_bits(&self) -> u64 {
        debug_assert!(
            !self.is_safe_for_any_store_load_forward_distances(),
            "Expected the distance, that prevent store-load forwarding, to be set."
        );
        self.max_store_load_forward_safe_distance_in_bits
    }

    /// `shouldRetryWithRuntimeChecks`: in some cases when the dependency check
    /// fails we can still vectorize the loop with a dynamic array access
    /// check.
    pub fn should_retry_with_runtime_checks(&self) -> bool {
        self.should_retry_with_runtime_checks && self.status == VectorizationSafetyStatus::PossiblySafeWithRtChecks
    }

    /// `getDependences`: none if we exceeded the `MaxDependences` threshold.
    pub fn dependences(&self) -> Option<&[Dependence]> {
        self.record_dependences.then_some(self.dependences.as_slice())
    }

    /// `clearDependences`.
    pub fn clear_dependences(&mut self) {
        self.dependences.clear();
    }

    /// `getMemoryInstructions`: the memory access instructions in program
    /// order; the indices identify instructions in a [`Dependence`].
    pub fn memory_instructions(&self) -> &[InstId] {
        &self.inst_map
    }

    /// `Dependence::getSource`.
    pub fn source(
        &self,
        dependence: &Dependence,
    ) -> InstId {
        self.inst_map[dependence.source as usize]
    }

    /// `Dependence::getDestination`.
    pub fn destination(
        &self,
        dependence: &Dependence,
    ) -> InstId {
        self.inst_map[dependence.destination as usize]
    }

    /// `getInnermostLoop`: the header.
    pub fn innermost_loop(&self) -> BlockId {
        self.innermost_loop
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2008
    /// `couldPreventStoreLoadForward`: whether a dependence at `distance`
    /// bytes between a store and a later load could prevent store-to-load
    /// forwarding when vectorized. `common_stride` 0 is none.
    fn could_prevent_store_load_forward(
        &mut self,
        distance: u64,
        type_byte_size: u64,
        common_stride: u64,
    ) -> bool {
        // If loads occur at a distance that is not a multiple of a feasible
        // vector factor store-load forwarding does not take place. Positive
        // dependences might cause troubles because vectorizing them might
        // prevent store-load forwarding making vectorized code run a lot
        // slower.
        //   a[i] = a[i-3] ^ a[i-8];
        //   The stores to a[i:i+1] don't align with the stores to a[i-3:i-2]
        //   and hence on your typical architecture store-load forwarding does
        //   not take place. Vectorizing in such cases does not make sense.
        // Store-load forwarding distance.

        // Maximum vector factor.
        let mut max_vf_without_sl_forward_issues_power_of_2 =
            (MAX_VECTOR_WIDTH * type_byte_size).min(self.max_store_load_forward_safe_distance_in_bits);

        // Compute the smallest VF at which the store and load would be
        // misaligned and recent enough to still be in the store buffer.
        let mut vf = 2 * type_byte_size;
        while vf <= max_vf_without_sl_forward_issues_power_of_2 {
            if is_store_load_forwarding_conflict(distance, vf, type_byte_size, vf) {
                max_vf_without_sl_forward_issues_power_of_2 = vf >> 1;
                break;
            }
            vf *= 2;
        }

        if max_vf_without_sl_forward_issues_power_of_2 < 2 * type_byte_size {
            return true;
        }

        if common_stride != 0
            && max_vf_without_sl_forward_issues_power_of_2 < self.max_store_load_forward_safe_distance_in_bits
            && max_vf_without_sl_forward_issues_power_of_2 != MAX_VECTOR_WIDTH * type_byte_size
        {
            let max_vf = bit_floor(max_vf_without_sl_forward_issues_power_of_2 / common_stride);
            let max_vf_in_bits = max_vf * type_byte_size * 8;
            self.max_store_load_forward_safe_distance_in_bits =
                self.max_store_load_forward_safe_distance_in_bits.min(max_vf_in_bits);

            if max_vf < 2 {
                return true;
            }
        }
        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2064
    /// `mergeInStatus`.
    fn merge_in_status(
        &mut self,
        s: VectorizationSafetyStatus,
    ) {
        if self.status < s {
            self.status = s;
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2165
    /// `areAccessesCompletelyBeforeOrAfter`: whether `sink` only accesses
    /// memory after `src`'s end or vice versa.
    fn are_accesses_completely_before_or_after(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        caches: &mut SharedCaches,
        src: Scev,
        src_ty: llrm_mir::types::TypeId,
        sink: Scev,
        sink_ty: llrm_mir::types::TypeId,
    ) -> bool {
        let lp = self.innermost_loop;
        let could_not_compute = se.get_could_not_compute();
        let (src_start, src_end) = get_start_and_end_for_access(
            se,
            cx,
            lp,
            src,
            src_ty,
            self.btc,
            self.symbolic_max_btc,
            Some(caches.pointer_bounds),
            caches.loop_guards,
        );
        if src_start == could_not_compute || src_end == could_not_compute {
            return false;
        }

        let (sink_start, sink_end) = get_start_and_end_for_access(
            se,
            cx,
            lp,
            sink,
            sink_ty,
            self.btc,
            self.symbolic_max_btc,
            Some(caches.pointer_bounds),
            caches.loop_guards,
        );
        if sink_start == could_not_compute || sink_end == could_not_compute {
            return false;
        }

        if caches.loop_guards.is_none() {
            *caches.loop_guards = Some(LoopGuards::collect(se, cx, lp));
        }
        let guards = caches.loop_guards.clone().expect("collected above");

        let src_end = se.apply_loop_guards_with(cx, src_end, &guards);
        let sink_start = se.apply_loop_guards_with(cx, sink_start, &guards);
        if se.is_known_predicate_scev(cx, IntPredicate::Ule, src_end, sink_start) {
            return true;
        }

        let sink_end = se.apply_loop_guards_with(cx, sink_end, &guards);
        let src_start = se.apply_loop_guards_with(cx, src_start, &guards);
        se.is_known_predicate_scev(cx, IntPredicate::Ule, sink_end, src_start)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2200
    /// `getDependenceDistanceStrideAndSize`: the distance, strides and sizes
    /// of the accesses `a` and `b` (instructions `a_inst`, `b_inst`), or the
    /// dependence type when they say nothing.
    fn get_dependence_distance_stride_and_size(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        a: MemAccessInfo,
        mut a_inst: InstId,
        b: MemAccessInfo,
        mut b_inst: InstId,
    ) -> Result<DepDistanceStrideAndSizeInfo, DepType> {
        let lp = self.innermost_loop;
        let (a_ptr, a_is_write) = a;
        let (b_ptr, b_is_write) = b;

        // Two reads are independent.
        if !a_is_write && !b_is_write {
            return Err(DepType::NoDep);
        }

        let mut a_ty = load_store_type(cx, a_inst).expect("a load or a store");
        let mut b_ty = load_store_type(cx, b_inst).expect("a load or a store");

        // We cannot check pointers in different address spaces.
        if crate::loopaccess::address_space(cx, a_ptr) != crate::loopaccess::address_space(cx, b_ptr) {
            return Err(DepType::Unknown);
        }

        let strides = SymbolicStrideMap::default();
        let mut stride_a_ptr = get_ptr_stride(se, cx, a_ty, a_ptr, lp, &strides, true);
        let mut stride_b_ptr = get_ptr_stride(se, cx, b_ty, b_ptr, lp, &strides, true);

        let mut src = se.get_scev(cx, a_ptr);
        let mut sink = se.get_scev(cx, b_ptr);

        // If the induction step is negative we have to invert source and sink
        // of the dependence when measuring the distance between them. We
        // should not swap AIsWrite with BIsWrite, as their uses expect them in
        // program order.
        if stride_a_ptr.is_some_and(|stride| stride < 0) {
            std::mem::swap(&mut src, &mut sink);
            std::mem::swap(&mut a_inst, &mut b_inst);
            std::mem::swap(&mut a_ty, &mut b_ty);
            std::mem::swap(&mut stride_a_ptr, &mut stride_b_ptr);
        }
        let _ = (a_inst, b_inst);

        let dist = se.get_minus_scev(cx, Use::of(sink), Use::of(src), 0, 0);

        // Need accesses with constant strides and the same direction for
        // further dependence analysis. We don't want to vectorize "A[B[i]] +=
        // ..." and similar code or pointer arithmetic that could wrap in the
        // address space.

        // If either Src or Sink are not strided (i.e. not a non-wrapping
        // AddRec) and not loop-invariant (stride will be 0 in that case), we
        // cannot analyze the dependence further and also cannot generate
        // runtime checks.
        let (Some(stride_a), Some(stride_b)) = (stride_a_ptr, stride_b_ptr) else {
            return Err(DepType::IndirectUnsafe);
        };

        // At least Src or Sink are loop invariant and the other is strided or
        // invariant.
        if stride_a == 0 || stride_b == 0 {
            // If both are loop-invariant and access the same location, we
            // cannot vectorize.
            if stride_a == 0 && stride_b == 0 && se.as_constant(dist).is_some_and(|d| d.bits == 0) {
                return Err(DepType::InvariantUnsafe);
            }
            // Otherwise, we can generate a runtime check to disambiguate the
            // accesses.
            return Err(DepType::Unknown);
        }

        // Both Src and Sink have a constant stride, check if they are in the
        // same direction.
        if (stride_a > 0) != (stride_b > 0) {
            return Err(DepType::Unknown);
        }

        let a_store_size = cx.layout.store_size(&cx.context.types, a_ty);
        let b_store_size = cx.layout.store_size(&cx.context.types, b_ty);

        // If store sizes are not the same, set TypeByteSize to zero, so we can
        // check it in the caller isDependent.
        let a_size = cx.layout.alloc_size(&cx.context.types, a_ty);
        let b_size = cx.layout.alloc_size(&cx.context.types, b_ty);
        let type_byte_size = if a_store_size == b_store_size { b_size } else { 0 };

        let stride_a_scaled = stride_a.unsigned_abs() * a_size;
        let stride_b_scaled = stride_b.unsigned_abs() * b_size;

        let max_stride = stride_a_scaled.max(stride_b_scaled);

        let common_stride = (stride_a_scaled == stride_b_scaled).then_some(stride_a_scaled);

        // TODO: Historically, we didn't retry with runtime checks when
        // (unscaled) strides were different but there is no inherent reason
        // to.
        if se.as_constant(dist).is_none() {
            self.should_retry_with_runtime_checks |= stride_a == stride_b;
        }

        // If distance is a SCEVCouldNotCompute, return Unknown immediately.
        if se.is_could_not_compute(dist) {
            return Err(DepType::Unknown);
        }

        Ok(DepDistanceStrideAndSizeInfo { dist, max_stride, common_stride, type_byte_size, a_is_write, b_is_write })
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2318
    /// `isDependent`: the dependence between the access `a` (the `a_idx`th
    /// memory instruction) and the later access `b`.
    fn is_dependent(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        caches: &mut SharedCaches,
        a: MemAccessInfo,
        a_idx: u32,
        b: MemAccessInfo,
        b_idx: u32,
    ) -> DepType {
        debug_assert!(a_idx < b_idx, "Must pass arguments in program order");
        let lp = self.innermost_loop;

        // Check if we can prove that Sink only accesses memory after Src's end
        // or vice versa. The helper is used to perform the checks only on the
        // exit paths where it helps to improve the analysis result.
        macro_rules! check_completely_before_or_after {
            () => {{
                let a_ty = load_store_type(cx, self.inst_map[a_idx as usize]).expect("a load or a store");
                let b_ty = load_store_type(cx, self.inst_map[b_idx as usize]).expect("a load or a store");
                let src = se.get_scev(cx, a.0);
                let sink = se.get_scev(cx, b.0);
                self.are_accesses_completely_before_or_after(se, cx, caches, src, a_ty, sink, b_ty)
            }};
        }

        // Get the dependence distance, stride, type size and what access
        // writes for the dependence between A and B.
        let a_inst = self.inst_map[a_idx as usize];
        let b_inst = self.inst_map[b_idx as usize];
        let info = match self.get_dependence_distance_stride_and_size(se, cx, a, a_inst, b, b_inst) {
            Err(ty) => {
                if ty == DepType::Unknown && check_completely_before_or_after!() {
                    return DepType::NoDep;
                }
                return ty;
            }
            Ok(info) => info,
        };

        let DepDistanceStrideAndSizeInfo {
            mut dist,
            max_stride,
            common_stride,
            type_byte_size,
            a_is_write,
            b_is_write,
        } = info;
        let has_same_size = type_byte_size > 0;

        // If the distance between the accesses is larger than their maximum
        // absolute stride multiplied by the symbolic maximum backedge taken
        // count (which is an upper bound of the number of iterations), the
        // accesses are independent, i.e. they are far enough apart that
        // accesses won't access the same location across all loop iterations.
        if has_same_size && is_safe_dependence_distance(se, cx, self.symbolic_max_btc, dist, max_stride) {
            return DepType::NoDep;
        }

        let ap_dist = se.as_constant(dist);
        let mut const_dist: u64 = 0;
        if let Some(distance) = ap_dist {
            // `APDist->abs().tryZExtValue()`.
            let width = distance.width;
            let signed = llrm_mir::context::signed(distance.bits, width);
            let magnitude = signed.unsigned_abs();
            let Ok(value) = u64::try_from(magnitude) else {
                return DepType::Unknown;
            };
            const_dist = value;
        }

        // Attempt to prove strided accesses independent.
        if ap_dist.is_some() {
            // If the distance between accesses and their strides are known
            // constants, check whether the accesses interlace each other.
            if const_dist > 0
                && common_stride.is_some_and(|stride| stride > 1)
                && has_same_size
                && are_strided_accesses_independent(const_dist, common_stride.expect("some"), type_byte_size)
            {
                return DepType::NoDep;
            }
        } else {
            if caches.loop_guards.is_none() {
                *caches.loop_guards = Some(LoopGuards::collect(se, cx, lp));
            }
            let guards = caches.loop_guards.clone().expect("collected above");
            dist = se.apply_loop_guards_with(cx, dist, &guards);
        }

        // Negative distances are not plausible dependencies.
        if se.is_known_non_positive(cx, dist) {
            if se.is_known_non_negative(cx, dist) {
                // Equal-sized accesses to the same location are forward.
                if has_same_size {
                    return DepType::Forward;
                }

                if common_stride.is_some() {
                    // For mixed sizes, CommonStride is asserted to cover both
                    // accesses when computed in
                    // getDependenceDistanceStrideAndSize, so different
                    // iterations cannot overlap.
                    return DepType::Forward;
                }
                return DepType::Unknown;
            }

            let is_true_data_dependence = a_is_write && !b_is_write;
            // Check if the first access writes to a location that is read in a
            // later iteration, where the distance between them is not a
            // multiple of a vector factor and relatively small.
            //
            // NOTE: There is no need to update MaxSafeVectorWidthInBits after
            // call to couldPreventStoreLoadForward, even if it changed
            // MinDepDistBytes, since a forward dependency will allow
            // vectorization using any width.
            if is_true_data_dependence && ENABLE_FORWARDING_CONFLICT_DETECTION {
                if const_dist == 0 {
                    return if check_completely_before_or_after!() { DepType::NoDep } else { DepType::Unknown };
                }
                if !has_same_size || self.could_prevent_store_load_forward(const_dist, type_byte_size, 0) {
                    return DepType::ForwardButPreventsForwarding;
                }
            }

            return DepType::Forward;
        }

        let min_distance = se.get_signed_range_min(cx, dist);
        let Ok(min_distance) = i64::try_from(llrm_mir::context::signed(min_distance.bits, min_distance.width)) else {
            return DepType::Unknown;
        };
        // Below we only handle strictly positive distances.
        if min_distance <= 0 {
            return if check_completely_before_or_after!() { DepType::NoDep } else { DepType::Unknown };
        }

        if !has_same_size {
            if check_completely_before_or_after!() {
                return DepType::NoDep;
            }
            return DepType::Unknown;
        }
        // Bail out early if passed-in parameters make vectorization not
        // feasible.
        let min_forced_factor = FORCED_FACTOR.max(1);
        let forced_unroll = if FORCED_INTERLEAVE != 0 { FORCED_INTERLEAVE } else { 1 };
        // The minimum number of iterations for a vectorized/unrolled version.
        let min_num_iter = (min_forced_factor * forced_unroll).max(2);

        // It's not vectorizable if the distance is smaller than the minimum
        // distance needed for a vectorized/unrolled version. Vectorizing one
        // iteration in front needs MaxStride. Vectorizing the last iteration
        // needs TypeByteSize. (No need to plus the last gap distance).
        //
        // We use MaxStride (maximum of src and sink strides) to get a
        // conservative lower bound on the MinDistanceNeeded in case of
        // different strides.

        // We know that Dist is positive, but it may not be constant. Use the
        // signed minimum for computations below, as this ensures we compute
        // the closest possible dependence distance.
        let min_distance_needed = max_stride * (u64::from(min_num_iter) - 1) + type_byte_size;
        if min_distance_needed > min_distance as u64 {
            if const_dist == 0 {
                // For non-constant distances, we checked the lower bound of
                // the dependence distance and the distance may be larger at
                // runtime (and safe for vectorization). Classify it as
                // Unknown, so we re-try with runtime checks, unless we can
                // prove both accesses cannot overlap.
                return if check_completely_before_or_after!() { DepType::NoDep } else { DepType::Unknown };
            }
            return DepType::Backward;
        }

        // Unsafe if the minimum distance needed is greater than smallest
        // dependence distance distance.
        if min_distance_needed > self.min_dep_dist_bytes {
            return DepType::Backward;
        }

        self.min_dep_dist_bytes = (min_distance as u64).min(self.min_dep_dist_bytes);

        let is_true_data_dependence = !a_is_write && b_is_write;
        if is_true_data_dependence
            && ENABLE_FORWARDING_CONFLICT_DETECTION
            && const_dist != 0
            && self.could_prevent_store_load_forward(
                min_distance as u64,
                type_byte_size,
                // `*CommonStride` of an empty optional is unspecified in C++
                // when the strides differ; none means no stride bound here.
                common_stride.unwrap_or(0),
            )
        {
            return DepType::BackwardVectorizableButPreventsForwarding;
        }

        let max_vf = self.min_dep_dist_bytes / max_stride;

        let max_vf_in_bits = max_vf * type_byte_size * 8;
        if const_dist == 0 && max_vf_in_bits < u64::from(self.max_target_vector_width_in_bits) {
            // For non-constant distances, we checked the lower bound of the
            // dependence distance and the distance may be larger at runtime
            // (and safe for vectorization). Classify it as Unknown, so we
            // re-try with runtime checks, unless we can prove both accesses
            // cannot overlap.
            return if check_completely_before_or_after!() { DepType::NoDep } else { DepType::Unknown };
        }

        if check_completely_before_or_after!() {
            return DepType::NoDep;
        }

        self.max_safe_vector_width_in_bits = self.max_safe_vector_width_in_bits.min(max_vf_in_bits);
        DepType::BackwardVectorizable
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2553
    /// `areDepsSafe`: whether the dependences between the accesses of the
    /// classes of `check_deps` are safe, recording them if so.
    pub fn are_deps_safe(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        caches: &mut SharedCaches,
        dep_cands: &DepCandidates,
        check_deps: &[MemAccessInfo],
    ) -> bool {
        self.min_dep_dist_bytes = u64::MAX;
        let mut visited: HashSet<MemAccessInfo> = HashSet::default();
        for &cur_access in check_deps {
            if visited.contains(&cur_access) {
                continue;
            }

            // Check accesses within this set.
            let members: Vec<MemAccessInfo> = dep_cands.members(&cur_access).copied().collect();

            // Check every access pair.
            for (ai, &access_i) in members.iter().enumerate() {
                visited.insert(access_i);
                let ai_is_write = access_i.1;
                // Reads from the same pointer don't create extra hazards, but
                // multiple stores do (WAW), so start from AI for writes and
                // next(AI) for reads.
                let first_other = if ai_is_write { ai } else { ai + 1 };
                for oi in first_other..members.len() {
                    let access_o = members[oi];
                    // Check every accessing instruction pair in program order.
                    let accesses_i = self.accesses.get(&access_i).cloned().unwrap_or_default();
                    for (i1, &idx1) in accesses_i.iter().enumerate() {
                        // When checking for WAW (OI == AI) caused by multiple
                        // writes to the same pointer, start I2 at the next
                        // access past I1 to avoid self-comparison.
                        let accesses_o: Vec<u32> = if oi == ai {
                            accesses_i[i1 + 1..].to_vec()
                        } else {
                            self.accesses.get(&access_o).cloned().unwrap_or_default()
                        };
                        for &idx2 in &accesses_o {
                            let mut pair_a = (access_i, idx1);
                            let mut pair_b = (access_o, idx2);

                            debug_assert!(idx1 != idx2);
                            if idx1 > idx2 {
                                std::mem::swap(&mut pair_a, &mut pair_b);
                            }

                            let ty = self.is_dependent(se, cx, caches, pair_a.0, pair_a.1, pair_b.0, pair_b.1);
                            self.merge_in_status(ty.is_safe_for_vectorization());

                            // Gather dependences unless we accumulated
                            // MaxDependences dependences. In that case return
                            // as soon as we find the first unsafe dependence.
                            // This puts a limit on this quadratic algorithm.
                            if self.record_dependences {
                                if ty != DepType::NoDep {
                                    self.dependences.push(Dependence { source: pair_a.1, destination: pair_b.1, ty });
                                }

                                if self.dependences.len() >= MAX_DEPENDENCES {
                                    self.record_dependences = false;
                                    self.dependences.clear();
                                }
                            }
                            if !self.record_dependences && !self.is_safe_for_vectorization() {
                                return false;
                            }
                        }
                    }
                }
            }
        }

        self.is_safe_for_vectorization()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2628
    /// `getInstructionsForAccess`: the instructions that read or write via
    /// `ptr`.
    pub fn instructions_for_access_of(
        &self,
        ptr: Operand,
        is_write: bool,
    ) -> Vec<InstId> {
        self.accesses
            .get(&(ptr, is_write))
            .map(|found| found.iter().map(|&idx| self.inst_map[idx as usize]).collect())
            .unwrap_or_default()
    }
}

impl AccessOrder for MemoryDepChecker {
    fn order_for_access(
        &self,
        ptr: Operand,
        is_write: bool,
    ) -> &[u32] {
        self.accesses.get(&(ptr, is_write)).map_or(&[], Vec::as_slice)
    }

    fn instructions_for_access(
        &self,
        ptr: Operand,
        is_write: bool,
    ) -> Vec<InstId> {
        self.instructions_for_access_of(ptr, is_write)
    }

    fn innermost_loop(&self) -> BlockId {
        self.innermost_loop
    }
}

/// `std::bit_floor`.
fn bit_floor(value: u64) -> u64 {
    if value == 0 { 0 } else { 1 << (63 - value.leading_zeros()) }
}

/// `MemoryDepChecker::isStoreLoadForwardingConflict`: whether a memory
/// dependence at byte distance `distance` between a store (with element size
/// `type_byte_size` bytes) widened to `vector_store_size` bytes and a
/// subsequent load of `load_element_size` bytes would prevent store-to-load
/// forwarding.
pub fn is_store_load_forwarding_conflict(
    distance: u64,
    vector_store_size: u64,
    type_byte_size: u64,
    load_element_size: u64,
) -> bool {
    debug_assert!(vector_store_size != 0, "Expected non-zero vector store size");
    let num_iters_for_store_load_through_memory = 8 * type_byte_size;
    if distance / vector_store_size >= num_iters_for_store_load_through_memory {
        return false;
    }
    let r = distance % vector_store_size;
    if r != 0 {
        return load_element_size > r;
    }
    load_element_size > vector_store_size
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2081
/// `isSafeDependenceDistance`: given a dependence-distance `dist` between two
/// memory accesses, that have strides in the same direction whose absolute
/// value of the maximum stride is given in `max_stride`, in a loop whose
/// maximum backedge taken count is `max_btc`, whether it is possible to prove
/// statically that the dependence distance is larger than the range that the
/// accesses will travel through the execution of the loop.
fn is_safe_dependence_distance(
    se: &mut ScalarEvolution,
    cx: &Cx,
    max_btc: Scev,
    dist: Scev,
    max_stride: u64,
) -> bool {
    // If we can prove that
    //      (**) |Dist| > MaxBTC * Step
    // where Step is the absolute stride of the memory accesses in bytes, then
    // there is no dependence.
    let btc_ty = se.type_of(max_btc);
    let step = se.get_constant_of_type(cx, btc_ty, i128::from(max_stride));
    let product = se.get_mul_expr(cx, vec![Use::of(max_btc), Use::of(step)], crate::scev::Flags::default(), 0).scev;

    let mut casted_dist = dist;
    let mut casted_product = product;
    let dist_type_size_bits = se.get_type_size_in_bits(cx, se.type_of(dist));
    let product_type_size_bits = se.get_type_size_in_bits(cx, se.type_of(product));

    // The dependence distance can be positive/negative, so we sign extend
    // Dist; The multiplication of the absolute stride in bytes and the
    // backedgeTakenCount is non-negative, so we zero extend Product.
    if dist_type_size_bits > product_type_size_bits {
        let dist_ty = se.type_of(dist);
        casted_product = se.get_zero_extend_expr(cx, Use::of(product), dist_ty, 0);
    } else {
        let product_ty = se.type_of(product);
        casted_dist = se.get_noop_or_sign_extend(cx, dist, product_ty);
    }

    // Is  Dist - (MaxBTC * Step) > 0 ? (If so, then we have proven (**)
    // because |Dist| >= Dist)
    let minus = se.get_minus_scev(cx, Use::of(casted_dist), Use::of(casted_product), 0, 0);
    if se.is_known_positive(cx, minus) {
        return true;
    }

    // Second try: Is  -Dist - (MaxBTC * Step) > 0 ? (If so, then we have
    // proven (**) because |Dist| >= -1*Dist)
    let neg_dist = se.get_negative_scev(cx, casted_dist, 0);
    let minus = se.get_minus_scev(cx, Use::of(neg_dist), Use::of(casted_product), 0, 0);
    se.is_known_positive(cx, minus)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L2136
/// `areStridedAccessesIndependent`: the dependence for two accesses with the
/// same stride `stride`; `distance` is the positive distance in bytes, and
/// `type_byte_size` is type size in bytes. True if they are independent.
fn are_strided_accesses_independent(
    distance: u64,
    stride: u64,
    type_byte_size: u64,
) -> bool {
    debug_assert!(stride > 1, "The stride must be greater than 1");
    debug_assert!(type_byte_size > 0, "The type size in byte must be non-zero");
    debug_assert!(distance > 0, "The distance must be non-zero");

    // Skip if the distance is not multiple of type byte size.
    if distance % type_byte_size != 0 {
        return false;
    }

    // No dependence if the distance is not multiple of the stride.
    // E.g.
    //      for (i = 0; i < 1024 ; i += 4)
    //        A[i+2] = A[i] + 1;
    //
    // Two accesses in memory (distance is 2, stride is 4):
    //     | A[0] |      |      |      | A[4] |      |      |      |
    //     |      |      | A[2] |      |      |      | A[6] |      |
    distance % stride != 0
}
