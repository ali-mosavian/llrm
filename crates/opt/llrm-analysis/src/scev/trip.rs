//! `BackedgeTakenInfo` and the trip counts read from it.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ScalarEvolution.cpp`: the exit
//! limit of each exiting block of a loop ([`ExitLimit`], found by
//! `exit_limit.rs`), gathered into the loop's [`BackedgeTakenInfo`] by
//! `computeBackedgeTakenCount`, kept per loop header in [`TripState`], and the
//! small constant trip counts and multiples asked of it.
//!
//! # Deviations
//! - Predicates (DEVIATE 1 of `docs/architecture/scev-port.md`): no
//!   `PredicatedBackedgeTakenCounts`, no `AllowPredicates`, and an
//!   `ExitNotTakenInfo` has no predicate list.
//!
//! # Example
//! `for (i = 0; i < n; i++)` has the exact backedge-taken count `(-1 + n)`.

use llrm_mir::dense::IdMap;
use llrm_mir::module::BlockId;

use super::expr::{Scev, Use};
use super::{Cx, ScalarEvolution};

/// `ScalarEvolution::ExitLimit`: how many times a loop's exit is not taken.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExitLimit {
    pub exact_not_taken: Scev,
    pub constant_max_not_taken: Scev,
    pub symbolic_max_not_taken: Scev,
    /// The backedge is taken either `constant_max_not_taken` or zero times.
    pub max_or_zero: bool,
    /// `Predicates`: the run-time checks the counts hold under (empty unless
    /// the limit was computed with `AllowPredicates`).
    pub predicates: Vec<super::wrap_predicate::Predicate>,
}

impl ExitLimit {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8949
    /// `ExitLimit(E)`.
    pub fn exact(
        se: &ScalarEvolution,
        e: Scev,
    ) -> Self {
        Self::new(se, e, e, e, false)
    }

    /// `ExitLimit()` of the default arguments: nothing is known.
    pub fn could_not_compute(se: &ScalarEvolution) -> Self {
        Self::exact(se, se.get_could_not_compute())
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8952
    /// `ExitLimit(E, ConstantMaxNotTaken, SymbolicMaxNotTaken, MaxOrZero)`.
    pub fn new(
        se: &ScalarEvolution,
        e: Scev,
        constant_max_not_taken: Scev,
        symbolic_max_not_taken: Scev,
        max_or_zero: bool,
    ) -> Self {
        let mut limit = Self {
            exact_not_taken: e,
            constant_max_not_taken,
            symbolic_max_not_taken,
            max_or_zero,
            predicates: Vec::new(),
        };
        // If we prove the max count is zero, so is the symbolic bound. This
        // happens in practice due to differences in a) how context sensitive
        // we've chosen to be and b) how we reason about bounds implied by UB.
        if se.match_zero(constant_max_not_taken) {
            limit.exact_not_taken = constant_max_not_taken;
            limit.symbolic_max_not_taken = constant_max_not_taken;
        }
        let unknown = se.get_could_not_compute();
        debug_assert!(
            limit.exact_not_taken == unknown || limit.constant_max_not_taken != unknown,
            "Exact is not allowed to be less precise than Constant Max"
        );
        debug_assert!(
            limit.exact_not_taken == unknown || limit.symbolic_max_not_taken != unknown,
            "Exact is not allowed to be less precise than Symbolic Max"
        );
        debug_assert!(
            limit.symbolic_max_not_taken == unknown || limit.constant_max_not_taken != unknown,
            "Symbolic Max is not allowed to be less precise than Constant Max"
        );
        debug_assert!(
            limit.constant_max_not_taken == unknown || se.match_apint(limit.constant_max_not_taken).is_some(),
            "No point in having a non-constant max backedge taken count!"
        );
        limit
    }

    /// `ExitLimit(..., Predicates)`.
    pub fn with_predicates(
        mut self,
        predicates: Vec<super::wrap_predicate::Predicate>,
    ) -> Self {
        self.predicates = predicates;
        self
    }

    /// `ExitLimit::hasFullInfo`.
    pub fn has_full_info(
        &self,
        se: &ScalarEvolution,
    ) -> bool {
        self.exact_not_taken != se.get_could_not_compute()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8994
    /// `ExitLimit::hasAnyInfo`.
    pub fn has_any_info(
        &self,
        se: &ScalarEvolution,
    ) -> bool {
        self.exact_not_taken != se.get_could_not_compute() || self.constant_max_not_taken != se.get_could_not_compute()
    }
}

/// `ScalarEvolution::ExitNotTakenInfo`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExitNotTakenInfo {
    pub exiting_block: BlockId,
    pub exact_not_taken: Scev,
    pub constant_max_not_taken: Scev,
    pub symbolic_max_not_taken: Scev,
    pub predicates: Vec<super::wrap_predicate::Predicate>,
}

/// `ScalarEvolution::BackedgeTakenInfo`.
#[derive(Clone, Debug)]
pub struct BackedgeTakenInfo {
    /// A list of computable exits and their not-taken counts.
    exit_not_taken: Vec<ExitNotTakenInfo>,
    /// The least constant maximum backedge-taken count of the loop that is
    /// known, or CouldNotCompute.
    constant_max: Scev,
    /// Whether `exit_not_taken` has an element for every exiting block.
    is_complete: bool,
    /// The least maximum backedge-taken count that is known, or
    /// CouldNotCompute. Lazily computed on first query.
    symbolic_max: Option<Scev>,
    /// The backedge is taken either `constant_max` or zero times.
    max_or_zero: bool,
}

impl BackedgeTakenInfo {
    /// `BackedgeTakenInfo()`: the invalid entry that stands while a count is
    /// being computed, so that a recursive request finds nothing to use.
    pub fn invalid(se: &ScalarEvolution) -> Self {
        Self {
            exit_not_taken: Vec::new(),
            constant_max: se.get_could_not_compute(),
            is_complete: false,
            symbolic_max: None,
            max_or_zero: false,
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9004
    /// `BackedgeTakenInfo(ExitCounts, IsComplete, ConstantMax, MaxOrZero)`.
    pub fn new(
        se: &ScalarEvolution,
        exit_counts: &[(BlockId, ExitLimit)],
        is_complete: bool,
        constant_max: Scev,
        max_or_zero: bool,
    ) -> Self {
        debug_assert!(
            constant_max == se.get_could_not_compute() || se.match_apint(constant_max).is_some(),
            "No point in having a non-constant max backedge taken count!"
        );
        Self {
            exit_not_taken: exit_counts
                .iter()
                .map(|(exiting_block, limit)| ExitNotTakenInfo {
                    exiting_block: *exiting_block,
                    exact_not_taken: limit.exact_not_taken,
                    constant_max_not_taken: limit.constant_max_not_taken,
                    symbolic_max_not_taken: limit.symbolic_max_not_taken,
                    predicates: limit.predicates.clone(),
                })
                .collect(),
            constant_max,
            is_complete,
            symbolic_max: None,
            max_or_zero,
        }
    }

    /// `hasAnyInfo`.
    pub fn has_any_info(
        &self,
        se: &ScalarEvolution,
    ) -> bool {
        !self.exit_not_taken.is_empty() || self.constant_max != se.get_could_not_compute()
    }

    /// `hasFullInfo`.
    pub fn has_full_info(&self) -> bool {
        self.is_complete
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8872
    /// `getExitNotTaken`.
    fn get_exit_not_taken(
        &self,
        exiting_block: BlockId,
    ) -> Option<&ExitNotTakenInfo> {
        self.exit_not_taken.iter().find(|one| one.exiting_block == exiting_block)
    }

    /// `getExact(ExitingBlock, SE)`.
    pub fn get_exact_for(
        &self,
        se: &ScalarEvolution,
        exiting_block: BlockId,
    ) -> Scev {
        self.get_exit_not_taken(exiting_block).map_or(se.get_could_not_compute(), |one| one.exact_not_taken)
    }

    /// `getConstantMax(ExitingBlock, SE)`.
    pub fn get_constant_max_for(
        &self,
        se: &ScalarEvolution,
        exiting_block: BlockId,
    ) -> Scev {
        self.get_exit_not_taken(exiting_block).map_or(se.get_could_not_compute(), |one| one.constant_max_not_taken)
    }

    /// `getSymbolicMax(ExitingBlock, SE)`.
    pub fn get_symbolic_max_for(
        &self,
        se: &ScalarEvolution,
        exiting_block: BlockId,
    ) -> Scev {
        self.get_exit_not_taken(exiting_block).map_or(se.get_could_not_compute(), |one| one.symbolic_max_not_taken)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8889
    /// `getConstantMax(SE)`: the constant max backedge taken count for the
    /// loop.
    pub fn get_constant_max(
        &self,
        se: &ScalarEvolution,
    ) -> Scev {
        if self.constant_max == se.get_could_not_compute() {
            return se.get_could_not_compute();
        }
        debug_assert!(
            se.match_apint(self.constant_max).is_some(),
            "No point in having a non-constant max backedge taken count!"
        );
        self.constant_max
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8941
    /// `isConstantMaxOrZero`.
    pub fn is_constant_max_or_zero(&self) -> bool {
        self.max_or_zero
    }
}

/// What `ScalarEvolution` keeps of trip counts: `BackedgeTakenCounts`, by the
/// header of the loop.
#[derive(Clone, Default)]
pub struct TripState {
    counts: IdMap<BlockId, BackedgeTakenInfo>,
    /// `PredicatedBackedgeTakenCounts`.
    predicated: IdMap<BlockId, BackedgeTakenInfo>,
    /// `LoopPropertiesCache`.
    properties: IdMap<BlockId, LoopProperties>,
}

/// `ScalarEvolution::LoopProperties`.
#[derive(Clone, Copy, Debug)]
pub struct LoopProperties {
    /// Loop has no abnormal exits: every instruction transfers execution to
    /// its successor.
    pub has_no_abnormal_exits: bool,
    /// Loop has no side effects that forward progress cares about.
    pub has_no_side_effects: bool,
}

impl TripState {
    /// `forgetBackedgeTakenCounts(L, /*Predicated=*/false)`.
    pub fn forget(
        &mut self,
        header: BlockId,
    ) {
        self.counts.remove(&header);
        self.predicated.remove(&header);
        self.properties.remove(&header);
    }

    /// `BackedgeTakenCounts.clear()`.
    pub fn clear(&mut self) {
        self.counts = IdMap::new();
        self.predicated = IdMap::new();
        self.properties = IdMap::new();
    }
}

thread_local! {
    static COMPUTATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many backedge-taken counts were worked out (a cache miss of
/// `getBackedgeTakenInfo`) on this thread: a test counts what a pass asks
/// afresh.
pub fn backedge_taken_computations() -> usize {
    COMPUTATIONS.with(std::cell::Cell::get)
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8587
    /// `getBackedgeTakenInfo(L)`: the loop headed by `header`.
    pub fn get_backedge_taken_info(
        &mut self,
        cx: &Cx,
        header: BlockId,
    ) -> BackedgeTakenInfo {
        // Initially insert an invalid entry for this loop. If the insertion
        // succeeds, proceed to actually compute a backedge-taken count and
        // update the value. The temporary CouldNotCompute value tells SCEV
        // code elsewhere that it shouldn't attempt to request a new
        // backedge-taken count, which could result in infinite recursion.
        if let Some(found) = self.trips.counts.get(&header) {
            return found.clone();
        }
        let invalid = BackedgeTakenInfo::invalid(self);
        self.trips.counts.insert(header, invalid);

        COMPUTATIONS.with(|one| one.set(one.get() + 1));
        let result = self.compute_backedge_taken_count(cx, header, false);

        // Now that we know more about the trip count for this loop, forget any
        // existing SCEV values for PHI nodes in this loop since they are only
        // conservative estimates made without the benefit of trip count
        // information. This invalidation is not necessary for correctness,
        // and is only done to produce more precise results.
        if result.has_any_info(self) {
            self.forget_loop_users(cx, header);
        }

        self.trips.counts.insert(header, result.clone());
        result
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8569
    /// `getPredicatedBackedgeTakenInfo(L)`: the count with the predicates it
    /// needs, where the unpredicated one is not complete.
    pub fn get_predicated_backedge_taken_info(
        &mut self,
        cx: &Cx,
        header: BlockId,
    ) -> BackedgeTakenInfo {
        let info = self.get_backedge_taken_info(cx, header);
        if info.has_full_info() {
            return info;
        }
        if let Some(found) = self.trips.predicated.get(&header) {
            return found.clone();
        }
        let invalid = BackedgeTakenInfo::invalid(self);
        self.trips.predicated.insert(header, invalid);
        let result = self.compute_backedge_taken_count(cx, header, true);
        self.trips.predicated.insert(header, result.clone());
        result
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9027
    /// `computeBackedgeTakenCount(L)`: the number of times the backedge of the
    /// loop will execute.
    fn compute_backedge_taken_count(
        &mut self,
        cx: &Cx,
        header: BlockId,
        allow_predicates: bool,
    ) -> BackedgeTakenInfo {
        let loop_ = cx.loops.get_loop_for(header).expect("the header of a loop");
        let exiting_blocks = loop_.get_exiting_blocks(cx.function);

        let mut exit_counts: Vec<(BlockId, ExitLimit)> = Vec::new();
        let mut could_compute_be_count = true;
        let latch = loop_.get_loop_latch(cx.function); // may be none.
        let mut must_exit_max_be_count: Option<Scev> = None;
        let mut may_exit_max_be_count: Option<Scev> = None;
        let mut must_exit_max_or_zero = false;
        let is_only_exit = exiting_blocks.len() == 1;
        let unknown = self.get_could_not_compute();

        // Compute the ExitLimit for each loop exit. Use this to populate
        // ExitCounts and compute maxBECount.
        for &exit_bb in &exiting_blocks {
            // We canonicalize untaken exits to br (constant), ignore them so
            // that proving an exit untaken doesn't negatively impact our
            // ability to reason about the loop as whole.
            if self.is_untaken_exit(cx, loop_, exit_bb) {
                continue;
            }

            let el = self.compute_exit_limit(cx, header, exit_bb, is_only_exit, allow_predicates);
            debug_assert!(
                allow_predicates || el.predicates.is_empty(),
                "Predicated exit limit when predicates are not allowed!"
            );

            // 1. For each exit that can be computed, add an entry to
            // ExitCounts. CouldComputeBECount is true only if all exits can be
            // computed.
            if el.exact_not_taken == unknown {
                // We couldn't compute an exact value for this exit, so we
                // won't be able to compute an exact value for the loop.
                could_compute_be_count = false;
            }
            // Remember exit count if either exact or symbolic is known.
            // Because Exact always implies symbolic, only check symbolic.
            if el.symbolic_max_not_taken != unknown {
                exit_counts.push((exit_bb, el.clone()));
            } else {
                debug_assert!(el.exact_not_taken == unknown, "Exact is known but symbolic isn't?");
            }

            // 2. Derive the loop's MaxBECount from each exit's max number of
            // non-exiting iterations. Partition the loop exits into two kinds:
            // LoopMustExits and LoopMayExits.
            //
            // If the exit dominates the loop latch, it is a LoopMustExit
            // otherwise it is a LoopMayExit. If any computable LoopMustExit is
            // found, then MaxBECount is the minimum EL.ConstantMaxNotTaken of
            // computable LoopMustExits. Otherwise, MaxBECount is
            // conservatively the maximum EL.ConstantMaxNotTaken, where
            // CouldNotCompute is considered greater than any computable
            // EL.ConstantMaxNotTaken.
            if el.constant_max_not_taken != unknown && latch.is_some_and(|latch| cx.tree.dominates(exit_bb, latch)) {
                match must_exit_max_be_count {
                    None => {
                        must_exit_max_be_count = Some(el.constant_max_not_taken);
                        must_exit_max_or_zero = el.max_or_zero;
                    }
                    Some(so_far) => {
                        must_exit_max_be_count =
                            Some(self.get_umin_from_mismatched_types_pair(cx, so_far, el.constant_max_not_taken));
                    }
                }
            } else if may_exit_max_be_count != Some(unknown) {
                if may_exit_max_be_count.is_none() || el.constant_max_not_taken == unknown {
                    may_exit_max_be_count = Some(el.constant_max_not_taken);
                } else {
                    may_exit_max_be_count = Some(self.get_umax_from_mismatched_types(
                        cx,
                        may_exit_max_be_count.expect("set"),
                        el.constant_max_not_taken,
                    ));
                }
            }
        }
        let max_be_count = must_exit_max_be_count.or(may_exit_max_be_count).unwrap_or(unknown);
        // The loop backedge will be taken the maximum or zero times if there's
        // a single exit that must be taken the maximum or zero times.
        let max_or_zero = must_exit_max_or_zero && exiting_blocks.len() == 1;

        BackedgeTakenInfo::new(self, &exit_counts, could_compute_be_count, max_be_count, max_or_zero)
    }

    /// Whether `exiting` ends in a conditional branch on a constant that
    /// never leaves the loop.
    fn is_untaken_exit(
        &self,
        cx: &Cx,
        loop_: llrm_mir::loopinfo::Loop,
        exiting: BlockId,
    ) -> bool {
        use llrm_mir::context::ConstantKind;
        use llrm_mir::module::Operand;
        use llrm_mir::opcode::Opcode;
        let Some(terminator) = cx.function.terminator(exiting) else { return false };
        let instruction = cx.function.instruction(terminator);
        let (Opcode::Br, [Operand::Constant(condition), Operand::Block(first), Operand::Block(_)]) =
            (&instruction.opcode, instruction.operands.as_slice())
        else {
            return false;
        };
        let zero = match cx.context.get(*condition).kind {
            ConstantKind::Int(bits) => bits & 1 == 0,
            ConstantKind::Zero => true,
            _ => return false,
        };
        let exit_if_true = !loop_.contains(*first);
        exit_if_true == zero
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4793
    /// `getUMinFromMismatchedTypes(LHS, RHS)`.
    pub fn get_umin_from_mismatched_types_pair(
        &mut self,
        cx: &Cx,
        lhs: Scev,
        rhs: Scev,
    ) -> Scev {
        self.get_umin_from_mismatched_types(cx, vec![Use::of(lhs), Use::of(rhs)], false)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4801
    /// `getUMinFromMismatchedTypes(Ops, Sequential)`.
    pub fn get_umin_from_mismatched_types(
        &mut self,
        cx: &Cx,
        ops: Vec<Use>,
        sequential: bool,
    ) -> Scev {
        debug_assert!(!ops.is_empty(), "At least one operand must be!");
        // Trivial case.
        if ops.len() == 1 {
            return ops[0].scev;
        }

        // Find the max type first.
        let mut max_type = self.type_of(ops[0].scev);
        for op in &ops[1..] {
            max_type = self.get_wider_type(cx, max_type, self.type_of(op.scev));
        }

        // Extend all ops to max type.
        let promoted: Vec<Use> = ops
            .iter()
            .map(|op| {
                let extended = self.get_noop_or_zero_extend(cx, op.scev, max_type);
                Use::of(extended)
            })
            .collect();

        // Generate umin.
        self.get_umin_expr(cx, promoted, sequential)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4780
    /// `getUMaxFromMismatchedTypes`.
    pub fn get_umax_from_mismatched_types(
        &mut self,
        cx: &Cx,
        lhs: Scev,
        rhs: Scev,
    ) -> Scev {
        let (mut promoted_lhs, mut promoted_rhs) = (lhs, rhs);
        let (left_ty, right_ty) = (self.type_of(lhs), self.type_of(rhs));
        if self.get_type_size_in_bits(cx, left_ty) > self.get_type_size_in_bits(cx, right_ty) {
            promoted_rhs = self.get_zero_extend_expr(cx, Use::of(rhs), left_ty, 0);
        } else {
            promoted_lhs = self.get_noop_or_zero_extend(cx, lhs, right_ty);
        }

        self.get_umax_expr(cx, vec![Use::of(promoted_lhs), Use::of(promoted_rhs)])
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7512
    /// `getLoopProperties`.
    pub fn get_loop_properties(
        &mut self,
        cx: &Cx,
        header: BlockId,
    ) -> LoopProperties {
        use llrm_mir::opcode::Opcode;
        use llrm_mir::valuetracking::guarantee::{
            is_guaranteed_to_transfer_execution_to_successor, may_throw, may_write_to_memory,
        };

        if let Some(found) = self.trips.properties.get(&header) {
            return *found;
        }
        let loop_ = cx.loops.get_loop_for(header).expect("the header of a loop");
        let query = cx.query(None);
        let has_side_effects = |inst| {
            let instruction = cx.function.instruction(inst);
            if let Opcode::Store { volatile, .. } = instruction.opcode {
                return volatile;
            }

            if may_throw(&query, inst) {
                return true;
            }

            // Non-volatile memset / memcpy do not count as side-effect for
            // forward progress.
            if query.callee_name(inst).is_some_and(|name| {
                ["llvm.memset.", "llvm.memcpy.", "llvm.memmove."].iter().any(|prefix| name.starts_with(prefix))
            }) && !matches!(
                instruction.operands.get(3),
                Some(llrm_mir::module::Operand::Constant(id))
                    if !matches!(cx.context.get(*id).kind, llrm_mir::context::ConstantKind::Int(0) | llrm_mir::context::ConstantKind::Zero)
            ) {
                return false;
            }

            may_write_to_memory(&query, inst)
        };

        let mut properties = LoopProperties { has_no_abnormal_exits: true, has_no_side_effects: true };
        'blocks: for &block in loop_.get_blocks() {
            for inst in cx.function.iter_block(block) {
                if !is_guaranteed_to_transfer_execution_to_successor(&query, inst) {
                    properties.has_no_abnormal_exits = false;
                }
                if has_side_effects(inst) {
                    properties.has_no_side_effects = false;
                }
                if !properties.has_no_abnormal_exits && !properties.has_no_side_effects {
                    // We're already as pessimistic as we can get.
                    break 'blocks;
                }
            }
        }

        self.trips.properties.insert(header, properties);
        properties
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1625
    /// `loopHasNoAbnormalExits`.
    pub fn loop_has_no_abnormal_exits(
        &mut self,
        cx: &Cx,
        header: BlockId,
    ) -> bool {
        self.get_loop_properties(cx, header).has_no_abnormal_exits
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1971
    /// `loopHasNoSideEffects`.
    pub fn loop_has_no_side_effects(
        &mut self,
        cx: &Cx,
        header: BlockId,
    ) -> bool {
        self.get_loop_properties(cx, header).has_no_side_effects
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7554
    /// `loopIsFiniteByAssumption`: a mustprogress loop without side effects
    /// must be finite.
    pub fn loop_is_finite_by_assumption(
        &mut self,
        cx: &Cx,
        header: BlockId,
    ) -> bool {
        let facts = llrm_mir::facts::Facts::of(&cx.function.attrs);
        let loop_ = cx.loops.get_loop_for(header).expect("the header of a loop");
        // isMustProgress(L): the function says so or the loop is marked.
        let marked = loop_
            .get_loop_latch(cx.function)
            .is_some_and(|latch| llrm_mir::loopinfo::marked(cx.metadata, cx.function, latch));
        // isFinite(L): the function returns (`willreturn`).
        facts.will_return() || ((facts.must_progress() || marked) && self.loop_has_no_side_effects(cx, header))
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8834
    /// `BackedgeTakenInfo::getExact(L, SE)`: the exact loop backedge taken
    /// count considering all loop exits. A computable result can only be
    /// returned for loops with all exiting blocks dominating the latch.
    /// howFarToZero assumes that the limit of each loop test is never skipped.
    /// This is a valid assumption as long as the loop exits via that test. For
    /// precise results, it is the caller's responsibility to specify the
    /// relevant loop exiting block using `get_exact_for`.
    pub(super) fn info_exact(
        &mut self,
        cx: &Cx,
        header: BlockId,
        info: &BackedgeTakenInfo,
    ) -> Scev {
        self.info_exact_with(cx, header, info, None)
    }

    /// `BackedgeTakenInfo::getExact(L, SE, Preds)`.
    pub(super) fn info_exact_with(
        &mut self,
        cx: &Cx,
        header: BlockId,
        info: &BackedgeTakenInfo,
        mut preds: Option<&mut Vec<super::wrap_predicate::Predicate>>,
    ) -> Scev {
        // If any exits were not computable, the loop is not computable.
        if !info.is_complete || info.exit_not_taken.is_empty() {
            return self.get_could_not_compute();
        }

        let loop_ = cx.loops.get_loop_for(header).expect("the header of a loop");
        // All exiting blocks we have collected must dominate the only
        // backedge.
        let Some(latch) = loop_.get_loop_latch(cx.function) else { return self.get_could_not_compute() };

        // All exiting blocks we have gathered dominate loop's latch, so exact
        // trip count is simply a minimum out of all these calculated exit
        // counts.
        let mut ops = Vec::new();
        for one in &info.exit_not_taken {
            debug_assert!(one.exact_not_taken != self.get_could_not_compute(), "Bad exit SCEV!");
            debug_assert!(
                cx.tree.dominates(one.exiting_block, latch),
                "We should only have known counts for exiting blocks that dominate latch!"
            );
            ops.push(Use::of(one.exact_not_taken));
            if let Some(preds) = preds.as_deref_mut() {
                preds.extend(one.predicates.iter().copied());
            }
            debug_assert!(preds.is_some() || one.predicates.is_empty(), "Predicate should be always true!");
        }

        // If an earlier exit exits on the first iteration (exit count zero),
        // then a later poison exit count should not propagate into the
        // result. This are exactly the semantics provided by umin_seq.
        self.get_umin_from_mismatched_types(cx, ops, true)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8908
    /// `BackedgeTakenInfo::getSymbolicMax(L, SE)`: the symbolic max backedge
    /// taken count for the loop, formed on first use.
    pub(super) fn info_symbolic_max(
        &mut self,
        cx: &Cx,
        header: BlockId,
    ) -> Scev {
        let info = self.get_backedge_taken_info(cx, header);
        if let Some(found) = info.symbolic_max {
            return found;
        }
        // Form an expression for the maximum exit count possible for this
        // loop. We merge the max and exact information to approximate a
        // version of getConstantMaxBackedgeTakenCount which isn't restricted
        // to just constants.
        let loop_ = cx.loops.get_loop_for(header).expect("the header of a loop");
        let unknown = self.get_could_not_compute();
        let mut exit_counts = Vec::new();
        for one in &info.exit_not_taken {
            let exit_count = one.symbolic_max_not_taken;
            if exit_count != unknown {
                debug_assert!(
                    loop_.get_loop_latch(cx.function).is_some_and(|latch| cx.tree.dominates(one.exiting_block, latch)),
                    "We should only have known counts for exiting blocks that dominate latch!"
                );
                exit_counts.push(Use::of(exit_count));
            }
        }
        let symbolic_max =
            if exit_counts.is_empty() { unknown } else { self.get_umin_from_mismatched_types(cx, exit_counts, true) };
        if let Some(held) = self.trips.counts.get_mut(&header) {
            held.symbolic_max = Some(symbolic_max);
        }
        symbolic_max
    }
}

impl BackedgeTakenInfo {
    /// `getExitNotTaken(ExitingBlock, Predicates)`.
    fn exit_not_taken_with(
        &self,
        exiting_block: BlockId,
        predicates: Option<&mut Vec<super::wrap_predicate::Predicate>>,
    ) -> Option<&ExitNotTakenInfo> {
        for one in &self.exit_not_taken {
            if one.exiting_block == exiting_block {
                if one.predicates.is_empty() {
                    return Some(one);
                } else if let Some(predicates) = predicates {
                    predicates.extend(one.predicates.iter().copied());
                    return Some(one);
                }
                return None;
            }
        }
        None
    }

    /// `getExact`, `getSymbolicMax` or `getConstantMax(ExitingBlock, SE,
    /// Predicates)`, by `kind`.
    pub fn exit_count_with(
        &self,
        se: &ScalarEvolution,
        exiting_block: BlockId,
        predicates: Option<&mut Vec<super::wrap_predicate::Predicate>>,
        kind: super::backedge::ExitCountKind,
    ) -> Scev {
        use super::backedge::ExitCountKind;
        self.exit_not_taken_with(exiting_block, predicates).map_or(se.get_could_not_compute(), |one| match kind {
            ExitCountKind::Exact => one.exact_not_taken,
            ExitCountKind::SymbolicMaximum => one.symbolic_max_not_taken,
            ExitCountKind::ConstantMaximum => one.constant_max_not_taken,
        })
    }

    /// `getConstantMax(SE, Predicates)`.
    pub fn constant_max_with(
        &self,
        se: &ScalarEvolution,
        predicates: &mut Vec<super::wrap_predicate::Predicate>,
    ) -> Scev {
        if self.constant_max == se.get_could_not_compute() {
            return self.constant_max;
        }
        for one in &self.exit_not_taken {
            predicates.extend(one.predicates.iter().copied());
        }
        self.constant_max
    }
}

impl ScalarEvolution {
    /// `getSymbolicMax(L, SE, Predicates)` of `info`, which is not cached
    /// here: the predicates are appended every time.
    pub(super) fn info_symbolic_max_with(
        &mut self,
        cx: &Cx,
        info: &BackedgeTakenInfo,
        predicates: &mut Vec<super::wrap_predicate::Predicate>,
    ) -> Scev {
        let unknown = self.get_could_not_compute();
        let mut exit_counts = Vec::new();
        for one in &info.exit_not_taken {
            if one.symbolic_max_not_taken != unknown {
                exit_counts.push(Use::of(one.symbolic_max_not_taken));
                predicates.extend(one.predicates.iter().copied());
            }
        }
        if exit_counts.is_empty() { unknown } else { self.get_umin_from_mismatched_types(cx, exit_counts, true) }
    }
}
