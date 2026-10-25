//! `RuntimePointerChecking`: the bounds a loop's pointers walk, grouped so a
//! few comparisons prove that the writes do not overlap the other accesses.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`
//! (`getStartAndEndForAccess`, `RuntimePointerChecking`,
//! `RuntimeCheckingPtrGroup`). `insert` records a pointer with the lowest
//! byte it touches in the loop and the highest plus one, from its SCEV and the
//! loop's backedge-taken counts. `generate_checks` groups the pointers of
//! each dependency class whose bounds differ by a constant, and every pair of
//! groups some write and a different dependency set make needs a check.
//! `crate::scev` expands the checks' bounds in `llrm-transforms`
//! (`addRuntimeChecks`).
//!
//! # Example
//! `a[i] = b[i]` over `n` trips: `a` is bounded by `[a, a + 4n)` and `b` by
//! `[b, b + 4n)`; the one check is that the two ranges do not meet.
//!
//! # DEVIATE 23 (no operand bundles)
//! MIR has no operand bundles, so `evaluatePtrAddRecAtMaxBTCWillNotWrap`'s
//! `getKnowledgeForValue(..., Dereferenceable, ...)` finds no assumption: the
//! dereferenceable bytes are the value's own. Exit: operand bundles in MIR.
//!
//! # DEVIATE 24 (no predicated SCEV)
//! `insert` takes the loop's backedge-taken counts where LLVM takes a
//! `PredicatedScalarEvolution`, which here adds no predicate (D1: SCEV has
//! none). Exit: PredicatedScalarEvolution, with `replaceSymbolicStrideSCEV`.

use llrm_mir::context::{Constant, ConstantExpr, ConstantKind};
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate};
use llrm_mir::types::TypeId;
use llrm_support::hash::HashMap;

use crate::equivalence::EquivalenceClasses;
use crate::scev::backedge::ExitCountKind;
use crate::scev::expr::{Kind, Scev, Ty, Use, flags};
use crate::scev::guards::LoopGuards;
use crate::scev::{Cx, Flags, ScalarEvolution};

thread_local! {
    static COMPARISONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many bounds `groupChecks` compared on this thread: a test counts the
/// work it did.
pub fn group_comparisons() -> usize {
    COMPARISONS.with(std::cell::Cell::get)
}

/// `VectorizerParams`, as the command line options of the pin set them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VectorizerParams {
    /// `-memory-check-merge-threshold`: comparisons made merging the bounds of
    /// pointers into groups.
    pub memory_check_merge_threshold: u32,
    /// `-hoist-runtime-checks`: a check in an inner loop is made over the
    /// range the outer loop covers where it can be.
    pub hoist_runtime_checks: bool,
}

impl Default for VectorizerParams {
    fn default() -> Self {
        Self { memory_check_merge_threshold: 100, hoist_runtime_checks: true }
    }
}

/// The bounds of an access already worked out, by the pointer's SCEV and the
/// size of the element (`DenseMap<pair<SCEV, SCEV>, pair<SCEV, SCEV>>`).
pub type PointerBounds = HashMap<(Scev, Scev), (Scev, Scev)>;

/// `MemoryDepChecker::MemAccessInfo`: a pointer and whether it is written.
pub type MemAccessInfo = (Operand, bool);

/// `MemoryDepChecker::DepCandidates`.
pub type DepCandidates = EquivalenceClasses<MemAccessInfo>;

/// What `RuntimePointerChecking` asks of `MemoryDepChecker`.
pub trait AccessOrder {
    /// `getOrderForAccess(Ptr, IsWrite)`: the positions of the accesses.
    fn order_for_access(
        &self,
        ptr: Operand,
        is_write: bool,
    ) -> &[u32];

    /// `getInstructionsForAccess(Ptr, IsWrite)`.
    fn instructions_for_access(
        &self,
        ptr: Operand,
        is_write: bool,
    ) -> Vec<InstId>;

    /// `getInnermostLoop`: the header.
    fn innermost_loop(&self) -> BlockId;
}

/// The pointer constant `inttoptr (iN value)` of address space `space`, or the
/// null pointer for 0, as `getSCEV` sees it: an unknown.
fn pointer_constant(
    cx: &Cx,
    space: u32,
    all_ones: bool,
) -> Operand {
    let ptr = cx.context.types.ptr(space);
    if !all_ones {
        return Operand::Constant(cx.context.constant(Constant { ty: ptr, kind: ConstantKind::Null }));
    }
    let int_ty = cx.context.types.int(cx.layout.pointer(space).index_bits);
    let ones = cx.context.int(int_ty, -1);
    Operand::Constant(cx.context.constant(Constant {
        ty: ptr,
        kind: ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value: ones }),
    }))
}

/// `addSCEVNoOverflow`: `a + b`, if it cannot unsigned wrap.
fn add_scev_no_overflow(
    se: &mut ScalarEvolution,
    cx: &Cx,
    a: Scev,
    b: Scev,
) -> Option<Scev> {
    se.will_not_overflow(cx, BinaryOp::Add, false, a, b, None)
        .then(|| se.get_add_expr(cx, vec![Use::of(a), Use::of(b)], Flags::default(), 0).scev)
}

/// `mulSCEVNoOverflow`: `a * b`, if it cannot unsigned wrap.
fn mul_scev_no_overflow(
    se: &mut ScalarEvolution,
    cx: &Cx,
    a: Scev,
    b: Scev,
) -> Option<Scev> {
    se.will_not_overflow(cx, BinaryOp::Mul, false, a, b, None)
        .then(|| se.get_mul_expr(cx, vec![Use::of(a), Use::of(b)], Flags::default(), 0).scev)
}

/// Whether `s` is the constant zero.
fn is_zero(
    se: &ScalarEvolution,
    s: Scev,
) -> bool {
    se.match_apint(s).is_some_and(|value| value.is_zero())
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L209
/// `evaluatePtrAddRecAtMaxBTCWillNotWrap`: whether evaluating `ar` at
/// `max_btc` cannot wrap, because `ar` there is inbounds of the object it
/// walks.
fn evaluate_ptr_add_rec_at_max_btc_will_not_wrap(
    se: &mut ScalarEvolution,
    cx: &Cx,
    ar: Scev,
    mut max_btc: Scev,
    elt_size: Scev,
    loop_guards: &mut Option<LoopGuards>,
) -> bool {
    let start = se.addrec_start(ar);
    let pointer_base = se.get_pointer_base(start);
    let Kind::Unknown(start_ptr_v) = se.node(pointer_base).kind.clone() else { return false };
    let Kind::AddRec(_, l) = se.node(ar).kind.clone() else { unreachable!("an add recurrence") };
    // We can ignore frees, as the fact that an object of a certain size existed
    // at the location *at some point* is sufficient to derive the nowrap fact.
    let (mut deref_bytes, can_be_null) =
        llrm_mir::valuetracking::known::pointer_dereferenceable_bytes(&cx.query(None), start_ptr_v);
    // If the deref size is only known when the pointer is non-null, ignore it
    // here and fall back to a dereferenceable assumption below.
    if deref_bytes != 0 && can_be_null {
        deref_bytes = 0;
    }

    let mut step = se.get_step_recurrence(cx, ar);
    let mut wider_ty = se.get_wider_type(cx, se.type_of(max_btc), se.type_of(step));
    let deref_bytes_scev = se.get_constant_of_type(cx, wider_ty, i128::from(deref_bytes));

    // D23: no dereferenceable assumption to widen `deref_bytes_scev` with.
    if is_zero(se, deref_bytes_scev) {
        return false;
    }

    let is_known_non_negative = se.is_known_non_negative(cx, step);
    if !is_known_non_negative && !se.is_known_negative(cx, step) {
        return false;
    }

    wider_ty = se.get_wider_type(cx, wider_ty, se.type_of(deref_bytes_scev));
    step = se.get_noop_or_sign_extend(cx, step, wider_ty);
    max_btc = se.get_noop_or_zero_extend(cx, max_btc, wider_ty);

    // For the computations below, make sure they don't unsigned wrap.
    // FIXME (LLVM): for a negative step the lowest accessed address is not
    // AR->getStart() but AR->evaluateAtIteration(MaxBTC, SE); the check below
    // therefore compares StartPtr against the highest accessed address instead
    // of the lowest.
    if !se.is_known_predicate_scev(cx, IntPredicate::Uge, start, pointer_base) {
        return false;
    }
    let offset = se.get_minus_scev(cx, Use::of(start), Use::of(pointer_base), 0, 0);
    let start_offset = se.get_noop_or_zero_extend(cx, offset, wider_ty);

    if loop_guards.is_none() {
        *loop_guards = Some(LoopGuards::collect(se, cx, l));
    }
    let guards = loop_guards.as_ref().expect("guards were just collected");
    max_btc = se.apply_loop_guards_with(cx, max_btc, guards);

    let abs_step = se.get_abs_expr(cx, step, false);
    // Total distance (in bytes) between the first and the last accessed
    // pointer.
    let mut dist_to_last_iter = mul_scev_no_overflow(se, cx, max_btc, abs_step);
    if dist_to_last_iter.is_none() {
        // Re-try with constant max backedge-taken count if using the symbolic
        // one failed.
        max_btc = se.get_backedge_taken_count(cx, l, ExitCountKind::ConstantMaximum);
        if max_btc == se.get_could_not_compute() {
            return false;
        }
        max_btc = se.get_noop_or_zero_extend(cx, max_btc, wider_ty);
        dist_to_last_iter = mul_scev_no_overflow(se, cx, max_btc, abs_step);
    }
    let Some(dist_to_last_iter) = dist_to_last_iter else { return false };

    // Total length in bytes of the accessed range (from the first accessed
    // byte through the end of the last access).
    let elt = se.get_noop_or_zero_extend(cx, elt_size, wider_ty);
    let Some(accessed_bytes) = add_scev_no_overflow(se, cx, dist_to_last_iter, elt) else { return false };

    // Compute MaxOffset per direction: exclusive upper offset of the accessed
    // range.
    let mut deref_bytes_scev = deref_bytes_scev;
    let max_offset;
    if is_known_non_negative {
        let Some(offset) = add_scev_no_overflow(se, cx, start_offset, accessed_bytes) else { return false };
        max_offset = offset;
        deref_bytes_scev = se.apply_loop_guards_with(cx, deref_bytes_scev, guards);
    } else {
        // FIXME (LLVM): two independent off-by-EltSize bugs on this branch:
        //  1. StartOffset here is actually the HIGHEST offset, because it is
        //     computed from AR->getStart() rather than
        //     AR->evaluateAtIteration(MaxBTC, SE) (see FIXME above).
        //  2. The lower check is over-strict by EltSize and the upper is
        //     under-counted by EltSize.
        debug_assert!(se.is_known_negative(cx, step), "must be known negative");
        if !se.is_known_predicate_scev(cx, IntPredicate::Sge, start_offset, accessed_bytes) {
            return false;
        }
        max_offset = start_offset;
    }
    // MaxOffset must not exceed the deref-region end.
    se.is_known_predicate_scev(cx, IntPredicate::Ule, max_offset, deref_bytes_scev)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L330
/// `isKnownNonDecreasingInLoop`: whether `s` is monotonically non-decreasing
/// (unsigned, without wrapping) across the iterations of loop `l`.
fn is_known_non_decreasing_in_loop(
    se: &mut ScalarEvolution,
    cx: &Cx,
    s: Scev,
    l: BlockId,
) -> bool {
    if se.is_loop_invariant(cx, s, Some(l)) {
        return true;
    }
    match se.node(s).kind.clone() {
        // Non-decreasing in the numerator when the divisor is loop-invariant.
        Kind::UDiv(lhs, rhs) => {
            se.is_loop_invariant(cx, rhs.scev, Some(l)) && is_known_non_decreasing_in_loop(se, cx, lhs.scev, l)
        }
        Kind::AddRec(_, header) => {
            debug_assert!(header == l, "trying to check for AddRec in different loop");
            se.get_monotonic_predicate_type(cx, s, IntPredicate::Uge)
                == Some(crate::scev::predicate::MonotonicPredicateType::MonotonicallyIncreasing)
        }
        _ => false,
    }
}

/// `getNonAffineMonotonicBounds`: bounds of a loop-variant pointer that is not
/// an affine add recurrence, if its offset is provably monotonically
/// non-decreasing. The range is half-open: `elt_size` is added to the address
/// of the last accessed element to form the end.
pub(crate) fn get_non_affine_monotonic_bounds(
    se: &mut ScalarEvolution,
    cx: &Cx,
    lp: BlockId,
    ptr_expr: Scev,
    elt_size: Scev,
) -> Option<(Scev, Scev)> {
    let Kind::Add(operands) = se.node(ptr_expr).kind.clone() else { return None };
    if se.get_no_wrap_flags(ptr_expr, flags::NUW) == 0 {
        return None;
    }
    let base = operands.iter().map(|operand| operand.scev).find(|&operand| se.type_of(operand).is_pointer())?;
    let could_not_compute = se.get_could_not_compute();
    if base == could_not_compute || !se.is_loop_invariant(cx, base, Some(lp)) {
        return None;
    }
    let offset = se.get_minus_scev(cx, Use::of(ptr_expr), Use::of(base), 0, 0);
    if offset == could_not_compute || !is_known_non_decreasing_in_loop(se, cx, offset, lp) {
        return None;
    }
    let off_start = se.split_into_init_and_post_inc(cx, lp, offset).0;
    let parent = cx.loops.get_loop_for(lp).and_then(|found| found.get_parent_loop()).map(|parent| parent.get_header());
    let off_end = se.get_scev_at_scope(cx, offset, parent);
    if off_start == could_not_compute
        || off_end == could_not_compute
        || !se.is_loop_invariant(cx, off_start, Some(lp))
        || !se.is_loop_invariant(cx, off_end, Some(lp))
    {
        return None;
    }
    let start = se.get_add_expr(cx, vec![Use::of(base), Use::of(off_start)], Flags::default(), 0).scev;
    let end = se.get_add_expr(cx, vec![Use::of(base), Use::of(off_end), Use::of(elt_size)], Flags::default(), 0).scev;
    Some((start, end))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L391
/// `getStartAndEndForAccess(Lp, PtrExpr, AccessTy, ...)`.
pub fn get_start_and_end_for_access(
    se: &mut ScalarEvolution,
    cx: &Cx,
    lp: BlockId,
    ptr_expr: Scev,
    access_ty: TypeId,
    btc: Scev,
    max_btc: Scev,
    bounds: Option<&mut PointerBounds>,
    loop_guards: &mut Option<LoopGuards>,
) -> (Scev, Scev) {
    let Ty::Ptr(space) = se.type_of(ptr_expr) else { unreachable!("a pointer expression") };
    let idx_ty = Ty::Int(cx.layout.pointer(space).index_bits);
    let elt_size = se.get_store_size_of_expr(cx, idx_ty, access_ty);
    get_start_and_end_for_access_sized(se, cx, lp, ptr_expr, elt_size, btc, max_btc, bounds, loop_guards)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L407
/// `getStartAndEndForAccess(Lp, PtrExpr, EltSizeSCEV, ...)`: the lowest
/// address the access touches in the loop and the highest plus the element
/// size, or could-not-compute twice.
pub fn get_start_and_end_for_access_sized(
    se: &mut ScalarEvolution,
    cx: &Cx,
    lp: BlockId,
    ptr_expr: Scev,
    elt_size: Scev,
    btc: Scev,
    max_btc: Scev,
    bounds: Option<&mut PointerBounds>,
    loop_guards: &mut Option<LoopGuards>,
) -> (Scev, Scev) {
    let could_not_compute = se.get_could_not_compute();
    if let Some(known) = bounds.as_ref().and_then(|cache| cache.get(&(ptr_expr, elt_size))) {
        return *known;
    }
    let result = start_and_end(se, cx, lp, ptr_expr, elt_size, btc, max_btc, loop_guards);
    if let Some(cache) = bounds {
        // The cache holds a failure as the pair the pin starts the entry with.
        cache.insert((ptr_expr, elt_size), result.unwrap_or((could_not_compute, could_not_compute)));
    }
    result.unwrap_or((could_not_compute, could_not_compute))
}

fn start_and_end(
    se: &mut ScalarEvolution,
    cx: &Cx,
    lp: BlockId,
    ptr_expr: Scev,
    elt_size: Scev,
    btc: Scev,
    max_btc: Scev,
    loop_guards: &mut Option<LoopGuards>,
) -> Option<(Scev, Scev)> {
    let could_not_compute = se.get_could_not_compute();
    // ScStart is the lowest accessed address; ScEnd is the highest one plus the
    // size of the accessed element.
    let (sc_start, sc_end);
    if se.is_loop_invariant(cx, ptr_expr, Some(lp)) {
        sc_start = ptr_expr;
        sc_end = se.get_add_expr(cx, vec![Use::of(ptr_expr), Use::of(elt_size)], Flags::default(), 0).scev;
    } else if let Kind::AddRec(..) = se.node(ptr_expr).kind {
        let ar = ptr_expr;
        let step = se.get_step_recurrence(cx, ar);
        // The address of the last accessed element, if it can be computed
        // precisely.
        let last_addr = if btc != could_not_compute {
            // Evaluating AR at an exact BTC is safe: LAA separately checks that
            // accesses cannot wrap in the loop. If evaluating AR at BTC wraps,
            // then the loop either triggers UB when executing a memory access
            // with a poison pointer or the wrapping/poisoned pointer is not
            // used.
            Some(se.evaluate_at_iteration(cx, ar, btc))
        } else if evaluate_ptr_add_rec_at_max_btc_will_not_wrap(se, cx, ar, max_btc, elt_size, loop_guards) {
            Some(se.evaluate_at_iteration(cx, ar, max_btc))
        } else {
            None
        };
        let start = se.addrec_start(ar);
        let Ty::Ptr(space) = se.type_of(ar) else { unreachable!("a pointer recurrence") };
        if se.is_known_negative(cx, step) {
            sc_start = match last_addr {
                Some(last) => last,
                None => {
                    let null = pointer_constant(cx, space, false);
                    se.get_unknown(cx, null)
                }
            };
            sc_end = se.get_add_expr(cx, vec![Use::of(start), Use::of(elt_size)], Flags::default(), 0).scev;
        } else if se.is_known_non_negative(cx, step) {
            sc_start = start;
            // The highest address for the type saturates; adding EltSize to it
            // would wrap to the start of the address space.
            sc_end = match last_addr {
                Some(last) => se.get_add_expr(cx, vec![Use::of(last), Use::of(elt_size)], Flags::default(), 0).scev,
                None => {
                    let ones = pointer_constant(cx, space, true);
                    se.get_unknown(cx, ones)
                }
            };
        } else {
            let last = last_addr?;
            // Fallback case: the step is not constant, but we can still get
            // the upper and lower bounds of the interval by using min/max
            // expressions.
            sc_start = se.get_umin_expr(cx, vec![Use::of(start), Use::of(last)], false);
            let top = se.get_umax_expr(cx, vec![Use::of(start), Use::of(last)]);
            sc_end = se.get_add_expr(cx, vec![Use::of(top), Use::of(elt_size)], Flags::default(), 0).scev;
        }
    } else {
        // The pointer is loop-variant but not an affine AddRec. Try to form a
        // tight bound for a monotonic offset.
        (sc_start, sc_end) = get_non_affine_monotonic_bounds(se, cx, lp, ptr_expr, elt_size)?;
    }

    debug_assert!(se.is_loop_invariant(cx, sc_start, Some(lp)), "ScStart needs to be invariant");
    debug_assert!(se.is_loop_invariant(cx, sc_end, Some(lp)), "ScEnd needs to be invariant");
    Some((sc_start, sc_end))
}

/// `RuntimePointerChecking::PointerInfo`.
#[derive(Clone, Debug)]
pub struct PointerInfo {
    /// The pointer we need to check.
    pub pointer_value: Operand,
    /// The smallest byte address accessed by the pointer throughout all
    /// iterations of the loop.
    pub start: Scev,
    /// The largest byte address accessed by the pointer throughout all
    /// iterations of the loop, plus 1.
    pub end: Scev,
    /// Whether this pointer is used for writing to memory.
    pub is_write_ptr: bool,
    /// The id of the set of pointers that could be dependent because of a
    /// shared underlying object.
    pub dependency_set_id: u32,
    /// The id of the disjoint alias set to which this pointer belongs.
    pub alias_set_id: u32,
    /// SCEV for the access.
    pub expr: Scev,
    /// Whether the pointer expressions needs to be frozen after expansion.
    pub needs_freeze: bool,
}

/// `RuntimeCheckingPtrGroup`: pointers one memcheck is required between.
#[derive(Clone, Debug)]
pub struct RuntimeCheckingPtrGroup {
    /// The SCEV expression which represents the upper bound of all the
    /// pointers in this group.
    pub high: Scev,
    /// The SCEV expression which represents the lower bound of all the
    /// pointers in this group.
    pub low: Scev,
    /// Indices of all the pointers that constitute this grouping.
    pub members: Vec<usize>,
    /// Address space of the involved pointers.
    pub address_space: u32,
    /// Whether the pointer needs to be frozen after expansion, e.g. because it
    /// may be poison outside the loop.
    pub needs_freeze: bool,
}

/// `PointerDiffInfo`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerDiffInfo {
    pub src_start: Scev,
    pub sink_start: Scev,
    pub access_size: u32,
    pub needs_freeze: bool,
}

/// A check between two groups, by their indices in `checking_groups`
/// (`RuntimePointerCheck`).
pub type RuntimePointerCheck = (usize, usize);

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L179
impl RuntimeCheckingPtrGroup {
    /// `RuntimeCheckingPtrGroup(Index, RtCheck)`: a group of the one pointer.
    fn new(
        cx: &Cx,
        index: usize,
        pointers: &[PointerInfo],
    ) -> Self {
        let pointer = &pointers[index];
        Self {
            high: pointer.end,
            low: pointer.start,
            members: vec![index],
            address_space: address_space_of(cx, pointer.pointer_value),
            needs_freeze: pointer.needs_freeze,
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L656
    /// `addPointer(Index, RtCheck)`.
    fn add_pointer(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        index: usize,
        pointers: &[PointerInfo],
    ) -> bool {
        let pointer = &pointers[index];
        let address_space = address_space_of(cx, pointer.pointer_value);
        self.add_pointer_bounds(se, cx, index, pointer.start, pointer.end, address_space, pointer.needs_freeze)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L664
    /// `addPointer(Index, Start, End, AS, NeedsFreeze, SE)`: the pointer joins
    /// the group if its bounds compare with the group's by a constant.
    pub fn add_pointer_bounds(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        index: usize,
        start: Scev,
        end: Scev,
        address_space: u32,
        needs_freeze: bool,
    ) -> bool {
        debug_assert!(
            self.address_space == address_space,
            "all pointers in a checking group must be in the same address space"
        );
        // Compare the starts and ends with the known minimum and maximum of
        // this set. We need to know how we compare against the min/max of the
        // set in order to be able to emit memchecks.
        let Some(min0) = min_from_exprs(se, cx, start, self.low) else { return false };
        let Some(min1) = min_from_exprs(se, cx, end, self.high) else { return false };
        // Update the low bound expression if we've found a new min value.
        if min0 == start {
            self.low = start;
        }
        // Update the high bound expression if we've found a new max value.
        if min1 != end {
            self.high = end;
        }
        self.members.push(index);
        self.needs_freeze |= needs_freeze;
        true
    }
}

/// The address space of the pointer `ptr`.
fn address_space_of(
    cx: &Cx,
    ptr: Operand,
) -> u32 {
    crate::loopaccess::address_space(cx, ptr).expect("a pointer")
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L648
/// `getMinFromExprs`: the lower of `i` and `j`, or none if their difference is
/// not a constant.
fn min_from_exprs(
    se: &mut ScalarEvolution,
    cx: &Cx,
    i: Scev,
    j: Scev,
) -> Option<Scev> {
    let diff = se.compute_constant_difference(cx, j, i)?;
    Some(if diff.is_negative() { j } else { i })
}

/// `RuntimePointerChecking`.
#[derive(Clone, Debug)]
pub struct RuntimePointerChecking {
    /// Whether we need to add the runtime check.
    pub need: bool,
    /// Information about the pointers that may require checking.
    pub pointers: Vec<PointerInfo>,
    /// A partitioning of pointers into "check groups".
    pub checking_groups: Vec<RuntimeCheckingPtrGroup>,
    /// The runtime checks required to establish independence of otherwise
    /// may-aliasing pointers in the loop.
    pub checks: Vec<RuntimePointerCheck>,
    /// Whether pointer-difference checks can be used.
    can_use_diff_check: bool,
    /// (pointer-difference, access size) pairs that can be used to prove that
    /// there are no vectorization-preventing dependencies.
    diff_checks: Vec<PointerDiffInfo>,
    params: VectorizerParams,
    /// `DC.getPointerBounds()`.
    pointer_bounds: PointerBounds,
    /// The cache for the loop guards of the loop.
    loop_guards: Option<LoopGuards>,
}

impl RuntimePointerChecking {
    pub fn new(params: VectorizerParams) -> Self {
        Self {
            need: false,
            pointers: Vec::new(),
            checking_groups: Vec::new(),
            checks: Vec::new(),
            can_use_diff_check: true,
            diff_checks: Vec::new(),
            params,
            pointer_bounds: PointerBounds::default(),
            loop_guards: None,
        }
    }

    /// What the dependence checker shares with the checks: the bounds already
    /// worked out, and the loop's guards.
    pub fn caches(&mut self) -> crate::memdepchecker::SharedCaches<'_> {
        crate::memdepchecker::SharedCaches {
            pointer_bounds: &mut self.pointer_bounds,
            loop_guards: &mut self.loop_guards,
        }
    }

    /// `reset`.
    pub fn reset(&mut self) {
        self.need = false;
        self.can_use_diff_check = true;
        self.pointers.clear();
        self.checks.clear();
        self.diff_checks.clear();
        self.checking_groups.clear();
    }

    /// `empty`: no run-time memory checking is necessary.
    pub fn is_empty(&self) -> bool {
        self.pointers.is_empty()
    }

    /// `getNumberOfChecks`.
    pub fn number_of_checks(&self) -> usize {
        self.checks.len()
    }

    /// `getDiffChecks`: (pointer-difference, access size) pairs that prove no
    /// vectorization-preventing dependency, or none if they cannot be used.
    pub fn diff_checks(&self) -> Option<&[PointerDiffInfo]> {
        self.can_use_diff_check.then_some(self.diff_checks.as_slice())
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L495
    /// `insert`: a pointer and its start and end SCEVs. False, inserting
    /// nothing, if the bounds of `ptr_expr` cannot be computed. `btc` and
    /// `symbolic_max_btc` are the loop's counts (D24).
    pub fn insert(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        lp: BlockId,
        ptr: Operand,
        ptr_expr: Scev,
        access_ty: TypeId,
        write_ptr: bool,
        dep_set_id: u32,
        as_id: u32,
        btc: Scev,
        symbolic_max_btc: Scev,
        needs_freeze: bool,
    ) -> bool {
        let (sc_start, sc_end) = get_start_and_end_for_access(
            se,
            cx,
            lp,
            ptr_expr,
            access_ty,
            btc,
            symbolic_max_btc,
            Some(&mut self.pointer_bounds),
            &mut self.loop_guards,
        );
        let could_not_compute = se.get_could_not_compute();
        if sc_start == could_not_compute || sc_end == could_not_compute {
            return false;
        }
        self.pointers.push(PointerInfo {
            pointer_value: ptr,
            start: sc_start,
            end: sc_end,
            is_write_ptr: write_ptr,
            dependency_set_id: dep_set_id,
            alias_set_id: as_id,
            expr: ptr_expr,
            needs_freeze,
        });
        true
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L512
    /// `tryToCreateDiffCheck`: a (pointer-difference, access size) pair for
    /// the groups `cgi` and `cgj`, or false, if pointer-difference checks
    /// cannot be used for them.
    fn try_to_create_diff_check(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        dc: &dyn AccessOrder,
        cgi: usize,
        cgj: usize,
    ) -> bool {
        // If either group contains multiple different pointers, bail out.
        // TODO (LLVM): Support multiple pointers by using the minimum or
        // maximum pointer, depending on src & sink.
        let (members_i, members_j) = (&self.checking_groups[cgi].members, &self.checking_groups[cgj].members);
        let ([src_index], [sink_index]) = (&members_i[..], &members_j[..]) else { return false };
        let (mut src, mut sink) = (self.pointers[*src_index].clone(), self.pointers[*sink_index].clone());

        // If either pointer is read and written, multiple checks may be
        // needed. Bail out.
        if !dc.order_for_access(src.pointer_value, !src.is_write_ptr).is_empty()
            || !dc.order_for_access(sink.pointer_value, !sink.is_write_ptr).is_empty()
        {
            return false;
        }
        let acc_src = dc.order_for_access(src.pointer_value, src.is_write_ptr);
        let acc_sink = dc.order_for_access(sink.pointer_value, sink.is_write_ptr);
        // If either pointer is accessed multiple times, there may not be a
        // clear src/sink relation. Bail out for now.
        let ([first_src], [first_sink]) = (acc_src, acc_sink) else { return false };

        // If the sink is accessed before src, swap src/sink.
        if first_sink < first_src {
            std::mem::swap(&mut src, &mut sink);
        }

        let inner_loop = dc.innermost_loop();
        let affine = |se: &ScalarEvolution, expr: Scev| match &se.node(expr).kind {
            Kind::AddRec(operands, header) if operands.len() == 2 && *header == inner_loop => {
                Some((operands[0].scev, operands[1].scev))
            }
            _ => None,
        };
        let Some((mut src_start, src_step)) = affine(se, src.expr) else { return false };
        let Some((mut sink_start, sink_step)) = affine(se, sink.expr) else { return false };
        let Some(step) = se.match_apint(src_step) else { return false };
        if sink_step != src_step {
            return false;
        }

        let src_insts = dc.instructions_for_access(src.pointer_value, src.is_write_ptr);
        let sink_insts = dc.instructions_for_access(sink.pointer_value, sink.is_write_ptr);
        let (Some(&src_inst), Some(&sink_inst)) = (src_insts.first(), sink_insts.first()) else { return false };
        let (Some(src_ty), Some(dst_ty)) =
            (crate::loopaccess::load_store_type(cx, src_inst), crate::loopaccess::load_store_type(cx, sink_inst))
        else {
            return false;
        };

        let types = &cx.context.types;
        let alloc_size = cx.layout.alloc_size(types, src_ty).max(cx.layout.alloc_size(types, dst_ty));

        // Only matching constant steps matching the AllocSize are supported at
        // the moment. This simplifies the difference computation. Can be
        // extended in the future.
        if step.sext_value().unsigned_abs() != u128::from(alloc_size) {
            return false;
        }

        // When counting down, the dependence distance needs to be swapped.
        if step.is_negative() {
            std::mem::swap(&mut sink_start, &mut src_start);
        }

        let sink_start_int = se.get_ptr_to_addr_expr(cx, sink_start);
        let src_start_int = se.get_ptr_to_addr_expr(cx, src_start);
        let could_not_compute = se.get_could_not_compute();
        if sink_start_int == could_not_compute || src_start_int == could_not_compute {
            return false;
        }

        // If the start values for both Src and Sink also vary according to an
        // outer loop, then it's probably better to avoid creating diff checks
        // because they may not be hoisted. We should instead let
        // addRuntimeChecks do the expanded full range overlap checks, which
        // can be hoisted.
        let parent = cx.loops.get_loop_for(inner_loop).and_then(|found| found.get_parent_loop());
        if let (true, Some(parent)) = (self.params.hoist_runtime_checks, parent) {
            if let (Kind::AddRec(_, src_loop), Kind::AddRec(_, sink_loop)) =
                (se.node(src_start_int).kind.clone(), se.node(sink_start_int).kind.clone())
            {
                // If the diff check would already be loop invariant (due to
                // the recurrences being the same), then we prefer to keep the
                // diff checks because they are cheaper.
                if src_loop == sink_loop
                    && src_loop == parent.get_header()
                    && se.get_step_recurrence(cx, src_start_int) != se.get_step_recurrence(cx, sink_start_int)
                {
                    return false;
                }
            }
        }

        self.diff_checks.push(PointerDiffInfo {
            src_start: src_start_int,
            sink_start: sink_start_int,
            access_size: u32::try_from(alloc_size).expect("an access size"),
            needs_freeze: src.needs_freeze || sink.needs_freeze,
        });
        true
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L630
    /// `generateChecks(DepCands)`: groups the pointers to reduce the number of
    /// memchecks necessary, then stores the checks.
    pub fn generate_checks(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        dc: &dyn AccessOrder,
        dep_cands: &DepCandidates,
    ) {
        debug_assert!(self.checks.is_empty(), "Checks is not empty");
        self.group_checks(se, cx, dep_cands);
        self.checks = self.make_checks(se, cx, dc);
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L613
    /// `generateChecks()`: a check for each pair of groups that
    /// `needs_checking`.
    fn make_checks(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        dc: &dyn AccessOrder,
    ) -> Vec<RuntimePointerCheck> {
        let mut checks = Vec::new();
        for i in 0..self.checking_groups.len() {
            for j in i + 1..self.checking_groups.len() {
                if self.groups_need_checking(i, j) {
                    self.can_use_diff_check =
                        self.can_use_diff_check && self.try_to_create_diff_check(se, cx, dc, i, j);
                    checks.push((i, j));
                }
            }
        }
        checks
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L637
    /// `needsChecking(M, N)`: whether a check is needed between two groups.
    pub fn groups_need_checking(
        &self,
        m: usize,
        n: usize,
    ) -> bool {
        self.checking_groups[m]
            .members
            .iter()
            .any(|&i| self.checking_groups[n].members.iter().any(|&j| self.needs_checking(i, j)))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L823
    /// `needsChecking(I, J)`: whether to issue a run-time check for the
    /// pointers at `i` and `j` to prove their independence.
    pub fn needs_checking(
        &self,
        i: usize,
        j: usize,
    ) -> bool {
        let (pointer_i, pointer_j) = (&self.pointers[i], &self.pointers[j]);
        // No need to check if two readonly pointers intersect.
        if !pointer_i.is_write_ptr && !pointer_j.is_write_ptr {
            return false;
        }
        // Only need to check pointers between two different dependency sets.
        if pointer_i.dependency_set_id == pointer_j.dependency_set_id {
            return false;
        }
        // Only need to check pointers in the same alias set.
        pointer_i.alias_set_id == pointer_j.alias_set_id
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L816
    /// `arePointersInSamePartition`: `ptr_to_partition` holds the partition
    /// number of each pointer (-1 if it belongs to several).
    pub fn are_pointers_in_same_partition(
        ptr_to_partition: &[i32],
        ptr_idx1: usize,
        ptr_idx2: usize,
    ) -> bool {
        ptr_to_partition[ptr_idx1] != -1 && ptr_to_partition[ptr_idx1] == ptr_to_partition[ptr_idx2]
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L695
    /// `groupChecks`: groups pointers such that a single memcheck is required
    /// between two different groups. It clears `checking_groups` and
    /// computes it again.
    fn group_checks(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        dep_cands: &DepCandidates,
    ) {
        // We build the groups from dependency candidates equivalence classes
        // because:
        //    - We know that pointers in the same equivalence class share the
        //      same underlying object and therefore there is a chance that we
        //      can compare pointers
        //    - We wouldn't be able to merge two pointers for which we need to
        //      emit a memcheck. The classes in DepCands are already
        //      conveniently built such that no two pointers in the same class
        //      need checking against each other.
        //
        // We use the following (greedy) algorithm to construct the groups.
        // For every pointer in the equivalence class:
        //   For each existing group:
        //   - if the difference between this pointer and the min/max bounds of
        //     the group is a constant, then make the pointer part of the group
        //     and update the min/max bounds of that group as required.
        //
        // If we need to check two pointers to the same underlying object with
        // a non-constant difference, we shouldn't perform any pointer grouping
        // with those pointers: the resulting check could return false, even
        // when the accesses are safe (a[5000 + i * m] = a[i] + a[i + 9000]).
        // Accesses not present in DepCands get separate groups.
        self.checking_groups.clear();
        let mut total_comparisons = 0u32;

        let mut position_map: HashMap<Operand, Vec<usize>> = HashMap::default();
        for (index, pointer) in self.pointers.iter().enumerate() {
            position_map.entry(pointer.pointer_value).or_default().push(index);
        }

        // The pointers already processed.
        let mut seen = vec![false; self.pointers.len()];

        // Go through all equivalence classes, get the "pointer check groups"
        // and add them to the overall solution. The order in which accesses
        // appear in `pointers` makes it deterministic.
        for i in 0..self.pointers.len() {
            // We've seen this pointer before, and therefore already processed
            // its equivalence class.
            if seen[i] {
                continue;
            }
            let access: MemAccessInfo = (self.pointers[i].pointer_value, self.pointers[i].is_write_ptr);

            // If there is no entry in the dependency partition, there are no
            // potential accesses to merge; simply add a new pointer checking
            // group.
            if !dep_cands.contains(&access) {
                let group = RuntimeCheckingPtrGroup::new(cx, i, &self.pointers);
                self.checking_groups.push(group);
                continue;
            }

            let mut groups: Vec<RuntimeCheckingPtrGroup> = Vec::new();
            for member in dep_cands.members(&access) {
                // If we can't find the pointer in the position map that means
                // we can't generate a memcheck for it.
                let Some(positions) = position_map.get(&member.0) else { continue };
                for &pointer in positions {
                    let mut merged = false;
                    // Mark this pointer as seen.
                    seen[pointer] = true;

                    // Go through all the existing sets and see if we can find
                    // one which can include this pointer.
                    for group in &mut groups {
                        // Don't perform more than a certain amount of
                        // comparisons. This limits the cost of grouping the
                        // pointers: past it the remaining pointers get groups
                        // of their own.
                        if total_comparisons > self.params.memory_check_merge_threshold {
                            break;
                        }
                        total_comparisons += 1;
                        COMPARISONS.with(|made| made.set(made.get() + 1));
                        if group.add_pointer(se, cx, pointer, &self.pointers) {
                            merged = true;
                            break;
                        }
                    }

                    if !merged {
                        // We couldn't add this pointer to any existing set or
                        // the threshold for the number of comparisons has been
                        // reached. Create a new group to hold the current
                        // pointer.
                        groups.push(RuntimeCheckingPtrGroup::new(cx, pointer, &self.pointers));
                    }
                }
            }

            // We've computed the grouped checks for this partition. Save the
            // results and continue with the next one.
            self.checking_groups.extend(groups);
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L848
    /// `printChecks`.
    pub fn print_checks(
        &self,
        text: &dyn ValueText,
        checks: &[RuntimePointerCheck],
        depth: usize,
    ) -> String {
        let mut out = String::new();
        let indent = |out: &mut String, by: usize| out.push_str(&" ".repeat(by));
        let pointers = |out: &mut String, group: &RuntimeCheckingPtrGroup| {
            for &k in &group.members {
                // `OS.indent(Depth + 2) << *PointerValue << "\n"`.
                indent(out, depth + 2);
                out.push_str(&text.value(self.pointers[k].pointer_value));
                out.push('\n');
            }
        };
        for (n, &(first, second)) in checks.iter().enumerate() {
            indent(&mut out, depth);
            out.push_str(&format!("Check {n}:\n"));
            indent(&mut out, depth + 2);
            out.push_str(&format!("Comparing group GRP{first}:\n"));
            pointers(&mut out, &self.checking_groups[first]);
            indent(&mut out, depth + 2);
            out.push_str(&format!("Against group GRP{second}:\n"));
            pointers(&mut out, &self.checking_groups[second]);
        }
        out
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L867
    /// `print`.
    pub fn print(
        &self,
        se: &ScalarEvolution,
        cx: &Cx,
        labels: &crate::scev::print::Labels,
        text: &dyn ValueText,
        depth: usize,
    ) -> String {
        let indent = " ".repeat(depth);
        let mut out = format!("{indent}Run-time memory checks:\n");
        out.push_str(&self.print_checks(text, &self.checks, depth));
        out.push_str(&format!("{indent}Grouped accesses:\n"));
        for (index, group) in self.checking_groups.iter().enumerate() {
            out.push_str(&format!("{indent}  Group GRP{index}:\n"));
            out.push_str(&format!(
                "{indent}    (Low: {} High: {})\n",
                se.print(cx, labels, group.low),
                se.print(cx, labels, group.high)
            ));
            for &member in &group.members {
                out.push_str(&format!("{indent}      Member: {}\n", se.print(cx, labels, self.pointers[member].expr)));
            }
        }
        out
    }
}

/// `operator<<(raw_ostream &, const Value &)`: how `opt` writes a value, the
/// instruction that defines it (with two spaces of its own) or an argument
/// (`ptr %s`).
pub trait ValueText {
    /// A value.
    fn value(
        &self,
        value: Operand,
    ) -> String;

    /// An instruction.
    fn instruction(
        &self,
        inst: InstId,
    ) -> String;
}

#[cfg(test)]
#[path = "runtimecheck_tests.rs"]
mod tests;
