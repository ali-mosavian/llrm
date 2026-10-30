//! What the conditions that guard a loop imply: `isLoopEntryGuardedByCond`,
//! `isLoopBackedgeGuardedByCond`, `isBasicBlockEntryGuardedByCond`,
//! `isKnownPredicateAt` and the `isImpliedCond*` family they call.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ScalarEvolution.cpp`. A predicate
//! between two expressions is guarded when a condition that dominates the
//! loop's entry (or the block, or controls its backedge) implies it. Climbing
//! from the block up through predecessors that have a unique successor into
//! it, each conditional branch's condition is asked whether, taken as found,
//! it implies the predicate: by matching operands, by ranges of constants, by
//! no-overflow reasoning, through a shift, a loop's start value, a matching
//! difference, or the operations that built the expressions.
//!
//! # Deviations
//! - D-G1 (temporary,
//!   [#2334](https://github.com/ali-mosavian/llrm/issues/2334)):
//!   `CmpPredicate`'s `samesign` is dropped, as the rest of the port's
//!   predicates do (`IntPredicate`), though MIR's `icmp` carries the flag.
//!   `CmpPredicate::getMatching` then matches fewer pairs, which is
//!   conservative.
//!
//! # Example
//! After `br (icmp ult %i, %n), loop, exit`, `%i u< %n` is guarded at the
//! loop's entry.

use llrm_mir::constantrange::ConstantRange;
use llrm_mir::module::{BlockId, GlobalKind, InstId, Operand};
use llrm_mir::opcode::{IntPredicate, Opcode};
use llrm_mir::valuetracking::pattern;

use super::ap::Ap;
use super::expr::{Kind, Scev, Use, flags};
use super::predicate::{is_equality, is_false_when_equal, is_relational, is_signed, is_true_when_equal, is_unsigned};
use super::{Cx, Flags, ScalarEvolution};

/// `MaxSCEVOperationsImplicationDepth`.
const MAX_SCEV_OPERATIONS_IMPLICATION_DEPTH: u32 = 2;

thread_local! {
    static GUARD_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many function bodies were read for guard calls on this thread: a test
/// counts what a module without the declaration costs.
pub fn guard_scans() -> usize {
    GUARD_SCANS.with(std::cell::Cell::get)
}

/// What the guard queries keep between calls: the conditions being analyzed,
/// the phis being merged, and whether a backedge query is on the stack.
#[derive(Clone, Default)]
pub struct GuardState {
    /// The calls of `llvm.experimental.guard`, found when first asked
    /// (`HasGuards` and the declaration's users).
    guard_calls: Option<Vec<InstId>>,
    /// `PendingLoopPredicates`.
    pending_loop_predicates: Vec<Operand>,
    /// `PendingMerges`.
    pending_merges: Vec<Operand>,
    /// `WalkingBEDominatingConds`.
    walking_be_dominating_conds: bool,
}

/// `ICmpInst::getNonStrictCmpPredicate`.
fn non_strict(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Sgt => IntPredicate::Sge,
        IntPredicate::Slt => IntPredicate::Sle,
        IntPredicate::Ugt => IntPredicate::Uge,
        IntPredicate::Ult => IntPredicate::Ule,
        other => other,
    }
}

/// `ICmpInst::getFlippedSignednessPredicate`.
fn flipped_signedness(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Sgt => IntPredicate::Ugt,
        IntPredicate::Sge => IntPredicate::Uge,
        IntPredicate::Slt => IntPredicate::Ult,
        IntPredicate::Sle => IntPredicate::Ule,
        IntPredicate::Ugt => IntPredicate::Sgt,
        IntPredicate::Uge => IntPredicate::Sge,
        IntPredicate::Ult => IntPredicate::Slt,
        IntPredicate::Ule => IntPredicate::Sle,
        other => other,
    }
}

fn is_lt(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Slt | IntPredicate::Ult)
}

fn is_le(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sle | IntPredicate::Ule)
}

fn is_gt(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sgt | IntPredicate::Ugt)
}

fn is_ge(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sge | IntPredicate::Uge)
}

/// `CmpPredicate::getMatching` without `samesign`.
fn matching(
    a: IntPredicate,
    b: IntPredicate,
) -> Option<IntPredicate> {
    (a == b).then_some(a)
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11377
    /// `haveSameSign`.
    pub fn have_same_sign(
        &mut self,
        cx: &Cx,
        s1: Scev,
        s2: Scev,
    ) -> bool {
        (self.is_known_non_negative(cx, s1) && self.is_known_non_negative(cx, s2))
            || (self.is_known_negative(cx, s1) && self.is_known_negative(cx, s2))
    }

    /// `HasGuards`: the function calls `llvm.experimental.guard`.
    pub fn has_guards(
        &mut self,
        cx: &Cx,
    ) -> bool {
        !self.guard_calls(cx).is_empty()
    }

    /// The calls of `llvm.experimental.guard` of the function (the users of
    /// its declaration).
    pub(super) fn guard_calls(
        &mut self,
        cx: &Cx,
    ) -> Vec<InstId> {
        use llrm_mir::valuetracking::pattern::{IntrinsicId, call_intrinsic};
        if self.guards.guard_calls.is_none() {
            let mut found = Vec::new();
            // The pin asks the module for the declaration; with none, no body
            // is read.
            let declared = cx
                .globals
                .iter()
                .any(
                    |global| matches!(global.kind, GlobalKind::Function(_))
                        && global.name.intrinsic_id() == Some(IntrinsicId::ExperimentalGuard),
                );
            if declared {
                GUARD_SCANS.with(|one| one.set(one.get() + 1));
                let query = cx.query(None);
                for &block in cx.function.layout() {
                    for inst in cx.function.iter_block(block) {
                        if call_intrinsic(&query, inst) == Some(IntrinsicId::ExperimentalGuard) {
                            found.push(inst);
                        }
                    }
                }
            }
            self.guards.guard_calls = Some(found);
        }
        self.guards
            .guard_calls
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|&inst| !cx.function.is_erased(inst))
            .collect()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11879
    /// `isImpliedViaGuard`: whether a guard in `bb` implies the predicate.
    fn is_implied_via_guard(
        &mut self,
        cx: &Cx,
        bb: BlockId,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> bool {
        // No need to even try if we know the module has no guards.
        if !self.has_guards(cx) {
            return false;
        }

        for guard in self.guard_calls(cx) {
            if cx.function.parent(guard) != Some(bb) {
                continue;
            }
            let condition = cx.function.instruction(guard).operands[0];
            if self.is_implied_cond(cx, pred, lhs, rhs, condition, false, None) {
                return true;
            }
        }
        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10940
    /// `getPredecessorWithUniqueSuccessorForBB`: the predecessor from which
    /// the block is reached, and the block (the loop's header for a loop's
    /// predecessor).
    pub(super) fn get_predecessor_with_unique_successor_for_bb(
        &self,
        cx: &Cx,
        bb: BlockId,
    ) -> Option<(BlockId, BlockId)> {
        // If the block has a unique predecessor, then there is no path from
        // the predecessor to the block that does not go through the direct
        // edge from the predecessor to the block.
        if let Some(pred) = single_predecessor(cx, bb) {
            return Some((pred, bb));
        }

        // A loop's header is defined to be a block that dominates the loop.
        // If the header has a unique predecessor outside the loop, it must be
        // a block that has exactly one successor that can reach the loop.
        if let Some(l) = cx.loops.get_loop_for(bb) {
            return l.get_loop_predecessor(cx.function).map(|pred| (pred, l.get_header()));
        }

        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11464
    /// `isKnownPredicateAt`.
    pub fn is_known_predicate_at(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        ctx_i: Option<InstId>,
    ) -> bool {
        // TODO (LLVM): Analyze guards and assumes from Context's block.
        self.is_known_predicate_scev(cx, pred, lhs, rhs)
            || ctx_i
                .and_then(|inst| cx.function.parent(inst))
                .is_some_and(|block| self.is_basic_block_entry_guarded_by_cond(cx, block, pred, lhs, rhs))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11898
    /// `isLoopBackedgeGuardedByCond`: whether the backedge of the loop is
    /// protected by a conditional between `lhs` and `rhs`. This is used to
    /// eliminate casts.
    pub fn is_loop_backedge_guarded_by_cond_pred(
        &mut self,
        cx: &Cx,
        l: BlockId,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> bool {
        let Some(loop_) = cx.loops.get_loop_for(l) else { return true };
        // Do not bother about unreachable loops.
        if !cx.tree.is_reachable(l) {
            return true;
        }

        if self.is_known_via_non_recursive_reasoning(cx, pred, lhs, rhs) {
            return true;
        }

        let Some(latch) = loop_.get_loop_latch(cx.function) else { return false };

        if let Some((condition, first)) = conditional_branch(cx, latch)
            && self.is_implied_cond(cx, pred, lhs, rhs, condition, first != l, None)
        {
            return true;
        }

        // We don't want more than one activation of the following loops on
        // the stack -- that can lead to O(n!) time complexity.
        if self.guards.walking_be_dominating_conds {
            return false;
        }

        self.guards.walking_be_dominating_conds = true;
        let proved = self.backedge_guarded_by_dominating_conds(cx, l, latch, pred, lhs, rhs);
        self.guards.walking_be_dominating_conds = false;
        proved
    }

    /// The part of `isLoopBackedgeGuardedByCond` that runs with
    /// `WalkingBEDominatingConds` set.
    fn backedge_guarded_by_dominating_conds(
        &mut self,
        cx: &Cx,
        l: BlockId,
        latch: BlockId,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> bool {
        // See if we can exploit a trip count to prove the predicate.
        let info = self.get_backedge_taken_info(cx, l);
        let latch_be_count = info.get_exact_for(self, latch);
        if latch_be_count != self.get_could_not_compute() {
            // We know that Latch branches back to the loop header exactly
            // LatchBECount times. This means the backdege condition at Latch
            // is equivalent to "{0,+,1} u< LatchBECount".
            let ty = self.type_of(latch_be_count);
            let nw_flags = flags::NUW | flags::NW;
            let zero = self.get_zero(cx, ty);
            let one = self.get_one(cx, ty);
            let loop_counter =
                self.get_add_rec_expr(cx, vec![Use::of(zero), Use::of(one)], l, Flags::expr(nw_flags)).scev;
            if self.is_implied_cond_preds(cx, pred, lhs, rhs, IntPredicate::Ult, loop_counter, latch_be_count, None) {
                return true;
            }
        }

        // Check conditions due to any @llvm.assume intrinsics.
        for &assume in &cx.assumptions.assumes {
            let Some(terminator) = cx.function.terminator(latch) else { continue };
            if !cx.tree.instruction_dominates(cx.function, assume, terminator) {
                continue;
            }

            let condition = cx.function.instruction(assume).operands[0];
            if self.is_implied_cond(cx, pred, lhs, rhs, condition, false, None) {
                return true;
            }
        }

        if self.is_implied_via_guard(cx, latch, pred, lhs, rhs) {
            return true;
        }

        let mut dtn = latch;
        while dtn != l {
            // should reach the loop header before reaching the root!
            let bb = dtn;

            if self.is_implied_via_guard(cx, bb, pred, lhs, rhs) {
                return true;
            }

            if let Some(pbb) = single_predecessor(cx, bb)
                && let Some((condition, first)) = conditional_branch(cx, pbb)
                && first != second_of(cx, pbb)
                // If we have an edge `E` within the loop body that dominates
                // the only latch, the condition guarding `E` also guards the
                // backedge. This reasoning works only for loops with a single
                // latch.
                && self.is_implied_cond(cx, pred, lhs, rhs, condition, bb != first, None)
            {
                return true;
            }

            let Some(idom) = cx.tree.immediate_dominator(bb) else { break };
            dtn = idom;
        }

        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11996
    /// `isBasicBlockEntryGuardedByCond`.
    pub fn is_basic_block_entry_guarded_by_cond(
        &mut self,
        cx: &Cx,
        bb: BlockId,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> bool {
        // Do not bother proving facts for unreachable code.
        if !cx.tree.is_reachable(bb) {
            return true;
        }

        // If we cannot prove strict comparison (e.g. a > b), maybe we can
        // prove the facts (a >= b && a != b) separately. A typical situation
        // is when the non-strict comparison is known from ranges and
        // non-equality is known from dominating predicates. If we are proving
        // strict comparison, we always try to prove non-equality and
        // non-strict comparison separately.
        let non_strict_predicate = non_strict(pred);
        let proving_strict_comparison = pred != non_strict_predicate;
        let mut proved_non_strict_comparison = false;
        let mut proved_non_equality = false;

        if proving_strict_comparison {
            if !proved_non_strict_comparison {
                proved_non_strict_comparison =
                    self.is_known_via_non_recursive_reasoning(cx, non_strict_predicate, lhs, rhs);
            }
            if !proved_non_equality {
                proved_non_equality = self.is_known_via_non_recursive_reasoning(cx, IntPredicate::Ne, lhs, rhs);
            }
            if proved_non_strict_comparison && proved_non_equality {
                return true;
            }
        }

        let ctx_i = cx.function.block(bb).first();

        // Try to prove (Pred, LHS, RHS) using isImpliedCond.
        let mut prove_via_cond = |se: &mut ScalarEvolution, condition: Operand, inverse: bool| -> bool {
            if se.is_implied_cond(cx, pred, lhs, rhs, condition, inverse, ctx_i) {
                return true;
            }
            if proving_strict_comparison {
                if !proved_non_strict_comparison {
                    proved_non_strict_comparison =
                        se.is_implied_cond(cx, non_strict_predicate, lhs, rhs, condition, inverse, ctx_i);
                }
                if !proved_non_equality {
                    proved_non_equality = se.is_implied_cond(cx, IntPredicate::Ne, lhs, rhs, condition, inverse, ctx_i);
                }
                if proved_non_strict_comparison && proved_non_equality {
                    return true;
                }
            }
            false
        };

        // Starting at the block's predecessor, climb up the predecessor chain,
        // as long as there are predecessors that can be found that have unique
        // successors leading to the original block.
        let containing_loop = cx.loops.get_loop_for(bb);
        let pred_bb = match containing_loop {
            Some(l) if l.get_header() == bb => l.get_loop_predecessor(cx.function),
            _ => single_predecessor(cx, bb),
        };
        let mut pair = pred_bb.map(|first| (first, bb));
        while let Some((first, second)) = pair {
            if let Some((condition, taken)) = conditional_branch(cx, first)
                && prove_via_cond(self, condition, taken != second)
            {
                return true;
            }
            pair = self.get_predecessor_with_unique_successor_for_bb(cx, first);
        }

        // Check conditions due to any @llvm.assume intrinsics.
        for &assume in &cx.assumptions.assumes {
            let Some(first) = cx.function.block(bb).first() else { continue };
            if !cx.tree.instruction_dominates(cx.function, assume, first) {
                continue;
            }

            let condition = cx.function.instruction(assume).operands[0];
            if prove_via_cond(self, condition, false) {
                return true;
            }
        }

        // Check conditions due to any @llvm.experimental.guard intrinsics.
        if let Some(first) = ctx_i {
            for guard in self.guard_calls(cx) {
                if cx.tree.instruction_dominates(cx.function, guard, first) {
                    let condition = cx.function.instruction(guard).operands[0];
                    if prove_via_cond(self, condition, false) {
                        return true;
                    }
                }
            }
        }
        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12096
    /// `isLoopEntryGuardedByCond`.
    pub fn is_loop_entry_guarded_by_cond_pred(
        &mut self,
        cx: &Cx,
        l: BlockId,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> bool {
        // Both LHS and RHS must be available at loop entry.
        debug_assert!(self.is_available_at_loop_entry(cx, lhs, l), "LHS is not available at Loop Entry");
        debug_assert!(self.is_available_at_loop_entry(cx, rhs, l), "RHS is not available at Loop Entry");

        if self.is_known_via_non_recursive_reasoning(cx, pred, lhs, rhs) {
            return true;
        }

        self.is_basic_block_entry_guarded_by_cond(cx, l, pred, lhs, rhs)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12116
    /// `isImpliedCond(Pred, LHS, RHS, FoundCondValue, Inverse, CtxI)`.
    pub fn is_implied_cond(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        found_cond_value: Operand,
        inverse: bool,
        ctx_i: Option<InstId>,
    ) -> bool {
        // False conditions implies anything. Do not bother analyzing it
        // further.
        if let Operand::Constant(id) = found_cond_value
            && let Some(bits) = match cx.context.get(id).kind {
                llrm_mir::context::ConstantKind::Int(bits) => Some(bits),
                llrm_mir::context::ConstantKind::Zero => Some(0),
                _ => None,
            }
            && (bits & 1 == 1) == inverse
        {
            return true;
        }

        if self.guards.pending_loop_predicates.contains(&found_cond_value) {
            return false;
        }
        self.guards.pending_loop_predicates.push(found_cond_value);
        let proved = self.is_implied_cond_value(cx, pred, lhs, rhs, found_cond_value, inverse, ctx_i);
        self.guards.pending_loop_predicates.retain(|one| *one != found_cond_value);
        proved
    }

    /// The body of `isImpliedCond` once the condition is pending.
    fn is_implied_cond_value(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        found_cond_value: Operand,
        inverse: bool,
        ctx_i: Option<InstId>,
    ) -> bool {
        let query = cx.query(None);
        // Recursively handle And and Or conditions.
        if let Some((op0, op1)) = pattern::logical_and(&query, found_cond_value) {
            if !inverse {
                return self.is_implied_cond(cx, pred, lhs, rhs, op0, inverse, ctx_i)
                    || self.is_implied_cond(cx, pred, lhs, rhs, op1, inverse, ctx_i);
            }
        } else if let Some((op0, op1)) = pattern::logical_or(&query, found_cond_value)
            && inverse
        {
            return self.is_implied_cond(cx, pred, lhs, rhs, op0, inverse, ctx_i)
                || self.is_implied_cond(cx, pred, lhs, rhs, op1, inverse, ctx_i);
        }

        let Some((_, instruction)) = query.instruction_of(found_cond_value) else { return false };
        let Opcode::ICmp(cmp) = instruction.opcode else { return false };

        // Now that we found a conditional branch that dominates the loop or
        // controls the loop latch. Check to see if it is the comparison we are
        // looking for.
        let found_pred = if inverse { cmp.inverse() } else { cmp };

        let found_lhs = self.get_scev(cx, instruction.operands[0]);
        let found_rhs = self.get_scev(cx, instruction.operands[1]);

        self.is_implied_cond_preds(cx, pred, lhs, rhs, found_pred, found_lhs, found_rhs, ctx_i)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12160
    /// `isImpliedCond(Pred, LHS, RHS, FoundPred, FoundLHS, FoundRHS, CtxI)`.
    pub fn is_implied_cond_preds(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
        found_pred: IntPredicate,
        mut found_lhs: Scev,
        mut found_rhs: Scev,
        ctx_i: Option<InstId>,
    ) -> bool {
        // Balance the types.
        let (lhs_bits, found_bits) = (
            self.get_type_size_in_bits(cx, self.type_of(lhs)),
            self.get_type_size_in_bits(cx, self.type_of(found_lhs)),
        );
        if lhs_bits < found_bits {
            // For unsigned and equality predicates, try to prove that both
            // found operands fit into narrow unsigned range. If so, try to
            // prove facts in narrow types.
            if !is_signed(found_pred) && !self.type_of(found_lhs).is_pointer() && !self.type_of(found_rhs).is_pointer()
            {
                let narrow_type = self.type_of(lhs);
                let wide_type = self.type_of(found_lhs);
                let max = self.get_constant(Ap::all_ones(lhs_bits));
                let max_value = self.get_zero_extend_expr(cx, Use::of(max), wide_type, 0);
                if self.is_known_via_non_recursive_reasoning(cx, IntPredicate::Ule, found_lhs, max_value)
                    && self.is_known_via_non_recursive_reasoning(cx, IntPredicate::Ule, found_rhs, max_value)
                {
                    let trunc_found_lhs = self.get_truncate_expr(cx, Use::of(found_lhs), narrow_type, 0);
                    let trunc_found_rhs = self.get_truncate_expr(cx, Use::of(found_rhs), narrow_type, 0);
                    // We cannot preserve samesign after truncation.
                    if self.is_implied_cond_balanced_types(
                        cx,
                        pred,
                        lhs,
                        rhs,
                        found_pred,
                        trunc_found_lhs,
                        trunc_found_rhs,
                        ctx_i,
                    ) {
                        return true;
                    }
                }
            }

            if self.type_of(lhs).is_pointer() || self.type_of(rhs).is_pointer() {
                return false;
            }
            let wide = self.type_of(found_lhs);
            if is_signed(pred) {
                lhs = self.get_sign_extend_expr(cx, Use::of(lhs), wide, 0);
                rhs = self.get_sign_extend_expr(cx, Use::of(rhs), wide, 0);
            } else {
                lhs = self.get_zero_extend_expr(cx, Use::of(lhs), wide, 0);
                rhs = self.get_zero_extend_expr(cx, Use::of(rhs), wide, 0);
            }
        } else if lhs_bits > found_bits {
            if self.type_of(found_lhs).is_pointer() || self.type_of(found_rhs).is_pointer() {
                return false;
            }
            let wide = self.type_of(lhs);
            if is_signed(found_pred) {
                found_lhs = self.get_sign_extend_expr(cx, Use::of(found_lhs), wide, 0);
                found_rhs = self.get_sign_extend_expr(cx, Use::of(found_rhs), wide, 0);
            } else {
                found_lhs = self.get_zero_extend_expr(cx, Use::of(found_lhs), wide, 0);
                found_rhs = self.get_zero_extend_expr(cx, Use::of(found_rhs), wide, 0);
            }
        }
        self.is_implied_cond_balanced_types(cx, pred, lhs, rhs, found_pred, found_lhs, found_rhs, ctx_i)
    }
}

/// `BasicBlock::getSinglePredecessor`: the block with the one edge into `bb`.
fn single_predecessor(
    cx: &Cx,
    bb: BlockId,
) -> Option<BlockId> {
    let mut found = None;
    for pred in llrm_mir::loopinfo::inverse_children(cx.function, bb) {
        if found.is_some() {
            return None;
        }
        found = Some(pred);
    }
    found
}

/// The condition of `block`'s conditional branch, and its first successor.
fn conditional_branch(
    cx: &Cx,
    block: BlockId,
) -> Option<(Operand, BlockId)> {
    let terminator = cx.function.terminator(block)?;
    let instruction = cx.function.instruction(terminator);
    match (&instruction.opcode, instruction.operands.as_slice()) {
        (Opcode::Br, [condition, Operand::Block(first), Operand::Block(_)]) => Some((*condition, *first)),
        _ => None,
    }
}

/// The second successor of `block`'s conditional branch.
fn second_of(
    cx: &Cx,
    block: BlockId,
) -> BlockId {
    let terminator = cx.function.terminator(block).expect("a terminator");
    match cx.function.instruction(terminator).operands[2] {
        Operand::Block(second) => second,
        _ => unreachable!("a conditional branch's second target"),
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12215
    /// `isImpliedCondBalancedTypes`.
    #[allow(clippy::too_many_arguments)]
    fn is_implied_cond_balanced_types(
        &mut self,
        cx: &Cx,
        mut pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
        mut found_pred: IntPredicate,
        mut found_lhs: Scev,
        mut found_rhs: Scev,
        ctx_i: Option<InstId>,
    ) -> bool {
        debug_assert!(
            self.get_type_size_in_bits(cx, self.type_of(lhs))
                == self.get_type_size_in_bits(cx, self.type_of(found_lhs)),
            "Types should be balanced!"
        );
        // Canonicalize the query to match the way instcombine will have
        // canonicalized the comparison.
        if self.simplify_icmp_operands(cx, &mut pred, &mut lhs, &mut rhs, 0) && lhs == rhs {
            return is_true_when_equal(pred);
        }
        if self.simplify_icmp_operands(cx, &mut found_pred, &mut found_lhs, &mut found_rhs, 0) && found_lhs == found_rhs
        {
            return is_false_when_equal(found_pred);
        }

        // Check to see if we can make the LHS or RHS match.
        if lhs == found_rhs || rhs == found_lhs {
            if self.as_constant(rhs).is_some() {
                std::mem::swap(&mut found_lhs, &mut found_rhs);
                found_pred = found_pred.swapped();
            } else {
                std::mem::swap(&mut lhs, &mut rhs);
                pred = pred.swapped();
            }
        }

        // Check whether the found predicate is the same as the desired
        // predicate.
        if let Some(p) = matching(found_pred, pred) {
            return self.is_implied_cond_operands(cx, p, lhs, rhs, found_lhs, found_rhs, ctx_i);
        }

        // Check whether swapping the found predicate makes it the same as the
        // desired predicate.
        if let Some(p) = matching(found_pred.swapped(), pred) {
            // We can write the implication
            // 0. LHS Pred      RHS  <-   FoundLHS SwapPred  FoundRHS
            // using one of the following ways:
            // 1. LHS Pred      RHS  <-   FoundRHS Pred      FoundLHS
            // 2. RHS SwapPred  LHS  <-   FoundLHS SwapPred  FoundRHS
            // 3. LHS Pred      RHS  <-  ~FoundLHS Pred     ~FoundRHS
            // 4. ~LHS SwapPred ~RHS  <-   FoundLHS SwapPred  FoundRHS
            // Forms 1. and 2. require swapping the operands of one condition.
            // Don't do this if it would break canonical constant/addrec
            // ordering.
            if self.as_constant(rhs).is_none() && !self.is_add_rec(lhs) {
                return self.is_implied_cond_operands(cx, p.swapped(), rhs, lhs, found_lhs, found_rhs, ctx_i);
            }
            if self.as_constant(found_rhs).is_none() && !self.is_add_rec(found_lhs) {
                return self.is_implied_cond_operands(cx, p, lhs, rhs, found_rhs, found_lhs, ctx_i);
            }

            // There's no clear preference between forms 3. and 4., try both.
            // Avoid forming getNotSCEV of pointer values as the resulting
            // subtract is not legal.
            if !self.type_of(lhs).is_pointer() && !self.type_of(rhs).is_pointer() {
                let (not_lhs, not_rhs) = (self.get_not_scev(cx, lhs), self.get_not_scev(cx, rhs));
                if self.is_implied_cond_operands(cx, p.swapped(), not_lhs, not_rhs, found_lhs, found_rhs, ctx_i) {
                    return true;
                }
            }

            if !self.type_of(found_lhs).is_pointer() && !self.type_of(found_rhs).is_pointer() {
                let (not_found_lhs, not_found_rhs) =
                    (self.get_not_scev(cx, found_lhs), self.get_not_scev(cx, found_rhs));
                if self.is_implied_cond_operands(cx, p, lhs, rhs, not_found_lhs, not_found_rhs, ctx_i) {
                    return true;
                }
            }

            return false;
        }

        let is_sign_flipped_predicate = |p1: IntPredicate, p2: IntPredicate| {
            debug_assert!(p1 != p2, "Handled earlier!");
            is_relational(p2) && p1 == flipped_signedness(p2)
        };
        if is_sign_flipped_predicate(pred, found_pred) {
            // Unsigned comparison is the same as signed comparison when both
            // the operands are non-negative or negative.
            if self.have_same_sign(cx, found_lhs, found_rhs) {
                return self.is_implied_cond_operands(cx, pred, lhs, rhs, found_lhs, found_rhs, ctx_i);
            }
            // Create local copies that we can freely swap and canonicalize our
            // conditions to "le/lt".
            let (mut canonical_pred, mut canonical_found_pred) = (pred, found_pred);
            let (mut canonical_lhs, mut canonical_rhs, mut canonical_found_lhs, mut canonical_found_rhs) =
                (lhs, rhs, found_lhs, found_rhs);
            if is_gt(canonical_pred) || is_ge(canonical_pred) {
                canonical_pred = canonical_pred.swapped();
                canonical_found_pred = canonical_found_pred.swapped();
                std::mem::swap(&mut canonical_lhs, &mut canonical_rhs);
                std::mem::swap(&mut canonical_found_lhs, &mut canonical_found_rhs);
            }
            debug_assert!(is_lt(canonical_pred) || is_le(canonical_pred), "Must be!");
            debug_assert!(is_lt(canonical_found_pred) || is_le(canonical_found_pred), "Must be!");
            if is_signed(canonical_pred) && self.is_known_non_negative(cx, canonical_rhs) {
                // Use implication:
                // x <u y && y >=s 0 --> x <s y.
                // If we can prove the left part, the right part is also
                // proven.
                return self.is_implied_cond_operands(
                    cx,
                    canonical_found_pred,
                    canonical_lhs,
                    canonical_rhs,
                    canonical_found_lhs,
                    canonical_found_rhs,
                    None,
                );
            }
            if is_unsigned(canonical_pred) && self.is_known_negative(cx, canonical_rhs) {
                // Use implication:
                // x <s y && y <s 0 --> x <u y.
                // If we can prove the left part, the right part is also
                // proven.
                return self.is_implied_cond_operands(
                    cx,
                    canonical_found_pred,
                    canonical_lhs,
                    canonical_rhs,
                    canonical_found_lhs,
                    canonical_found_rhs,
                    None,
                );
            }
        }

        // Check if we can make progress by sharpening ranges.
        if found_pred == IntPredicate::Ne
            && (self.as_constant(found_lhs).is_some() || self.as_constant(found_rhs).is_some())
        {
            let (c, v) = if let Some(c) = self.as_constant(found_lhs) {
                (c, found_rhs)
            } else {
                (self.as_constant(found_rhs).expect("a constant"), found_lhs)
            };

            // The guarding predicate tells us that C != V. If the known range
            // of V is [C, t), we can sharpen the range to [C + 1, t). The range
            // we consider has to correspond to same signedness as the
            // predicate we're interested in folding.
            let min =
                if is_signed(pred) { self.get_signed_range_min(cx, v) } else { self.get_unsigned_range_min(cx, v) };

            if min == c {
                // Given (V >= Min && V != Min) we conclude V >= (Min + 1).
                // This is true even if (Min + 1) wraps around -- in case of
                // wraparound, (Min + 1) < Min, so (V >= Min => V >= (Min + 1)).
                let sharper_min = min.add(Ap::one(min.width));
                let min_scev = self.get_constant(min);
                let sharper = self.get_constant(sharper_min);

                match pred {
                    IntPredicate::Sge | IntPredicate::Uge => {
                        // We know V `Pred` SharperMin. If this implies LHS
                        // `Pred` RHS, we're done.
                        if self.is_implied_cond_operands(cx, pred, lhs, rhs, v, sharper, ctx_i) {
                            return true;
                        }
                        // We know from the range information that
                        // (V `Pred` Min || V == Min). We know from the guarding
                        // condition that !(V == Min). If V `Pred` Min implies
                        // LHS `Pred` RHS, we're done.
                        if self.is_implied_cond_operands(cx, pred, lhs, rhs, v, min_scev, ctx_i) {
                            return true;
                        }
                    }
                    IntPredicate::Sgt | IntPredicate::Ugt => {
                        if self.is_implied_cond_operands(cx, pred, lhs, rhs, v, min_scev, ctx_i) {
                            return true;
                        }
                    }
                    // `LHS < RHS` and `LHS <= RHS` are handled in the same way
                    // as `RHS > LHS` and `RHS >= LHS` respectively.
                    IntPredicate::Sle | IntPredicate::Ule => {
                        if self.is_implied_cond_operands(cx, pred.swapped(), rhs, lhs, v, sharper, ctx_i) {
                            return true;
                        }
                        if self.is_implied_cond_operands(cx, pred.swapped(), rhs, lhs, v, min_scev, ctx_i) {
                            return true;
                        }
                    }
                    IntPredicate::Slt | IntPredicate::Ult => {
                        if self.is_implied_cond_operands(cx, pred.swapped(), rhs, lhs, v, min_scev, ctx_i) {
                            return true;
                        }
                    }
                    // No change
                    _ => {}
                }
            }
        }

        // Check whether the actual condition is beyond sufficient.
        if found_pred == IntPredicate::Eq
            && is_true_when_equal(pred)
            && self.is_implied_cond_operands(cx, pred, lhs, rhs, found_lhs, found_rhs, ctx_i)
        {
            return true;
        }
        if pred == IntPredicate::Ne
            && !is_true_when_equal(found_pred)
            && self.is_implied_cond_operands(cx, found_pred, lhs, rhs, found_lhs, found_rhs, ctx_i)
        {
            return true;
        }

        if self.is_implied_cond_operands_via_ranges(cx, pred, lhs, rhs, found_pred, found_lhs, found_rhs) {
            return true;
        }

        // Otherwise assume the worst.
        false
    }

    /// `isa<SCEVAddRecExpr>`.
    fn is_add_rec(
        &self,
        s: Scev,
    ) -> bool {
        matches!(self.node(s).kind, Kind::AddRec(..))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12878
    /// `isImpliedCondOperands`.
    pub fn is_implied_cond_operands(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        found_lhs: Scev,
        found_rhs: Scev,
        ctx_i: Option<InstId>,
    ) -> bool {
        self.is_implied_cond_operands_via_ranges(cx, pred, lhs, rhs, pred, found_lhs, found_rhs)
            || self.is_implied_cond_operands_via_no_overflow(cx, pred, lhs, rhs, found_lhs, found_rhs)
            || self.is_implied_cond_operands_via_shift(cx, pred, lhs, rhs, found_lhs, found_rhs)
            || self.is_implied_cond_operands_via_add_rec_start(cx, pred, lhs, rhs, found_lhs, found_rhs, ctx_i)
            || self.is_implied_cond_operands_via_matching_diff(cx, pred, lhs, rhs, found_lhs, found_rhs)
            || self.is_implied_cond_operands_helper(cx, pred, lhs, rhs, found_lhs, found_rhs)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L13268
    /// `isImpliedCondOperandsViaRanges`.
    #[allow(clippy::too_many_arguments)]
    fn is_implied_cond_operands_via_ranges(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        found_pred: IntPredicate,
        found_lhs: Scev,
        found_rhs: Scev,
    ) -> bool {
        // The restriction on `FoundRHS` be lifted easily -- it exists only to
        // reduce the compile time impact of this optimization.
        let (Some(const_rhs), Some(const_found_rhs)) = (self.as_constant(rhs), self.as_constant(found_rhs)) else {
            return false;
        };

        let Some(addend) = self.compute_constant_difference(cx, lhs, found_lhs) else { return false };

        // `FoundLHSRange` is the range we know `FoundLHS` to be in by virtue of
        // the antecedent "`FoundLHS` `FoundPred` `FoundRHS`".
        let found_lhs_range =
            ConstantRange::make_exact_icmp_region(found_pred, const_found_rhs.width, const_found_rhs.bits);

        // Since `LHS` is `FoundLHS` + `Addend`, we can compute a range for
        // `LHS`:
        let lhs_range = found_lhs_range.add(&ConstantRange::single(addend.width, addend.bits));

        // We can also compute the range of values for `LHS` that satisfy the
        // consequent, "`LHS` `Pred` `RHS`". The antecedent implies the
        // consequent if every value of `LHS` that satisfies the antecedent
        // also satisfies the consequent.
        lhs_range.icmp(pred, &ConstantRange::single(const_rhs.width, const_rhs.bits))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12591
    /// `isImpliedCondOperandsViaNoOverflow`.
    fn is_implied_cond_operands_via_no_overflow(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        found_lhs: Scev,
        found_rhs: Scev,
    ) -> bool {
        if pred != IntPredicate::Slt && pred != IntPredicate::Ult {
            return false;
        }

        if !self.is_add_rec(lhs) || !self.is_add_rec(found_lhs) {
            return false;
        }

        // We'd like to let SCEV reason about control dependencies, so we
        // constrain both the inequalities to be about add recurrences on the
        // same loop. This way we can use isLoopEntryGuardedByCond later.
        let l = self.addrec_header(found_lhs).expect("an add recurrence");
        if Some(l) != self.addrec_header(lhs) {
            return false;
        }

        // (LLVM's proof of (1) FoundLHS u< FoundRHS u< -C => (FoundLHS + C) u<
        // (FoundRHS + C) and (2) its signed form follows here in the source.)
        let Some(l_diff) = self.compute_constant_difference(cx, lhs, found_lhs) else { return false };
        let Some(r_diff) = self.compute_constant_difference(cx, rhs, found_rhs) else { return false };
        if l_diff != r_diff {
            return false;
        }

        if l_diff.is_zero() {
            return true;
        }

        let found_rhs_limit = if pred == IntPredicate::Ult {
            r_diff.neg()
        } else {
            let bits = self.get_type_size_in_bits(cx, self.type_of(rhs));
            Ap::signed_min(bits).sub(r_diff)
        };

        // Try to prove (1) or (2), as needed.
        let limit = self.get_constant(found_rhs_limit);
        self.is_available_at_loop_entry(cx, found_rhs, l)
            && self.is_loop_entry_guarded_by_cond_pred(cx, l, pred, found_rhs, limit)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12791
    /// `isImpliedCondOperandsViaShift`.
    fn is_implied_cond_operands_via_shift(
        &mut self,
        cx: &Cx,
        mut pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
        mut found_lhs: Scev,
        mut found_rhs: Scev,
    ) -> bool {
        // We want to imply LHS < RHS from LHS < (RHS >> shiftvalue). First,
        // make sure that we are dealing with same LHS.
        if rhs == found_rhs {
            std::mem::swap(&mut lhs, &mut rhs);
            std::mem::swap(&mut found_lhs, &mut found_rhs);
            pred = pred.swapped();
        }
        if lhs != found_lhs {
            return false;
        }

        let Some(value) = self.unknown_value(found_rhs) else { return false };

        let query = cx.query(None);
        let Some((_, shift)) = query.instruction_of(value) else { return false };
        if shift.opcode != Opcode::Binary(llrm_mir::opcode::BinaryOp::LShr) {
            return false;
        }
        let shiftee = shift.operands[0];
        let shiftee_s = self.get_scev(cx, shiftee);
        // Prove one of the following:
        // LHS <u (shiftee >> shiftvalue) && shiftee <=u RHS ---> LHS <u RHS
        // LHS <=u (shiftee >> shiftvalue) && shiftee <=u RHS ---> LHS <=u RHS
        // LHS <s (shiftee >> shiftvalue) && shiftee <=s RHS && shiftee >=s 0
        //   ---> LHS <s RHS
        // LHS <=s (shiftee >> shiftvalue) && shiftee <=s RHS && shiftee >=s 0
        //   ---> LHS <=s RHS
        if matches!(pred, IntPredicate::Ult | IntPredicate::Ule) {
            return self.is_known_predicate_scev(cx, IntPredicate::Ule, shiftee_s, rhs);
        }
        if matches!(pred, IntPredicate::Slt | IntPredicate::Sle) && self.is_known_non_negative(cx, shiftee_s) {
            return self.is_known_predicate_scev(cx, IntPredicate::Sle, shiftee_s, rhs);
        }

        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12544
    /// `isImpliedCondOperandsViaAddRecStart`.
    #[allow(clippy::too_many_arguments)]
    fn is_implied_cond_operands_via_add_rec_start(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        found_lhs: Scev,
        found_rhs: Scev,
        ctx_i: Option<InstId>,
    ) -> bool {
        // Try to recognize the following pattern:
        //
        //   FoundRHS = ...
        // ...
        // loop:
        //   FoundLHS = {Start,+,W}
        // context_bb: // Basic block from the same loop
        //   known(Pred, FoundLHS, FoundRHS)
        //
        // If some predicate is known in the context of a loop, it is also known
        // on each iteration of this loop, including the first iteration.
        // Therefore, in this case, `FoundLHS Pred FoundRHS` implies
        // `Start Pred FoundRHS`. Try to prove the original pred using this
        // fact.
        let Some(ctx) = ctx_i else { return false };
        let Some(context_bb) = cx.function.parent(ctx) else { return false };
        // Make sure AR varies in the context block.
        for (ar, other, is_left) in [(found_lhs, found_rhs, true), (found_rhs, found_lhs, false)] {
            let Some(header) = self.addrec_header(ar) else { continue };
            let l = cx.loops.get_loop_for(header).expect("the header of a loop");
            let latch = l.get_loop_latch(cx.function);
            // Make sure that context belongs to the loop and executes on 1st
            // iteration (if it ever executes at all).
            if !l.contains(context_bb) || !latch.is_some_and(|latch| cx.tree.dominates(context_bb, latch)) {
                return false;
            }
            if !self.is_available_at_loop_entry(cx, other, header) {
                return false;
            }
            let start = self.addrec_start(ar);
            return if is_left {
                self.is_implied_cond_operands(cx, pred, lhs, rhs, start, other, None)
            } else {
                self.is_implied_cond_operands(cx, pred, lhs, rhs, other, start, None)
            };
        }

        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12833
    /// `isImpliedCondOperandsViaMatchingDiff`.
    fn is_implied_cond_operands_via_matching_diff(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        found_lhs: Scev,
        found_rhs: Scev,
    ) -> bool {
        // Only valid for equality predicates: (A == B) implies (C == D) when
        // the SCEV difference A - B equals C - D (they check the same
        // underlying relationship at every iteration).
        if !is_equality(pred) {
            return false;
        }

        // Restrict to cases involving loop recurrences - that's where this
        // pattern arises (correlated IV comparisons). This avoids calling
        // getMinusSCEV on arbitrary non-loop expressions.
        if (!self.is_add_rec(lhs) && !self.is_add_rec(rhs))
            || (!self.is_add_rec(found_lhs) && !self.is_add_rec(found_rhs))
        {
            return false;
        }

        // AddRecs from different loops can never produce matching differences.
        let query_add_rec = if self.is_add_rec(lhs) { lhs } else { rhs };
        let found_add_rec = if self.is_add_rec(found_lhs) { found_lhs } else { found_rhs };
        if self.addrec_header(query_add_rec) != self.addrec_header(found_add_rec) {
            return false;
        }

        // If the strides differ, the differences can never match.
        if self.get_step_recurrence(cx, query_add_rec) != self.get_step_recurrence(cx, found_add_rec) {
            return false;
        }

        // Compute differences. For pointer-typed operands sharing the same
        // base, getMinusSCEV strips the common base and returns an integer
        // SCEV.
        let found_diff = self.get_minus_scev(cx, Use::of(found_lhs), Use::of(found_rhs), 0, 0);
        if found_diff == self.get_could_not_compute() {
            return false;
        }

        let diff = self.get_minus_scev(cx, Use::of(lhs), Use::of(rhs), 0, 0);
        if diff == self.get_could_not_compute() {
            return false;
        }

        diff == found_diff
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L13222
    /// `isImpliedCondOperandsHelper`.
    fn is_implied_cond_operands_helper(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        found_lhs: Scev,
        found_rhs: Scev,
    ) -> bool {
        use IntPredicate::*;
        let (a, b) = match pred {
            Eq | Ne => {
                if self.has_same_value(cx, lhs, found_lhs) && self.has_same_value(cx, rhs, found_rhs) {
                    return true;
                }
                (None, None)
            }
            Slt | Sle => (Some(Sle), Some(Sge)),
            Sgt | Sge => (Some(Sge), Some(Sle)),
            Ult | Ule => (Some(Ule), Some(Uge)),
            Ugt | Uge => (Some(Uge), Some(Ule)),
        };
        if let (Some(a), Some(b)) = (a, b)
            && self.is_known_via_non_recursive_reasoning(cx, a, lhs, found_lhs)
            && self.is_known_via_non_recursive_reasoning(cx, b, rhs, found_rhs)
        {
            return true;
        }

        // Maybe it can be proved via operations?
        self.is_implied_via_operations(cx, pred, lhs, rhs, found_lhs, found_rhs, 0)
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L13014
    /// `isImpliedViaOperations`.
    #[allow(clippy::too_many_arguments)]
    fn is_implied_via_operations(
        &mut self,
        cx: &Cx,
        mut pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
        mut found_lhs: Scev,
        mut found_rhs: Scev,
        depth: u32,
    ) -> bool {
        debug_assert!(
            self.get_type_size_in_bits(cx, self.type_of(lhs)) == self.get_type_size_in_bits(cx, self.type_of(rhs)),
            "LHS and RHS have different sizes?"
        );
        debug_assert!(
            self.get_type_size_in_bits(cx, self.type_of(found_lhs))
                == self.get_type_size_in_bits(cx, self.type_of(found_rhs)),
            "FoundLHS and FoundRHS have different sizes?"
        );
        // We want to avoid hurting the compile time with analysis of too big
        // trees.
        if depth > MAX_SCEV_OPERATIONS_IMPLICATION_DEPTH {
            return false;
        }

        // We only want to work with GT comparison so far.
        if is_lt(pred) {
            pred = pred.swapped();
            std::mem::swap(&mut lhs, &mut rhs);
            std::mem::swap(&mut found_lhs, &mut found_rhs);
        }

        // `getPreferredSignedPredicate` of a predicate without samesign.
        let mut p = pred;

        // For unsigned, try to reduce it to corresponding signed comparison.
        if p == IntPredicate::Ugt {
            // We can replace unsigned predicate with its signed counterpart if
            // all involved values are non-negative.
            // TODO (LLVM): We could have better support for unsigned.
            if self.is_known_non_negative(cx, found_lhs) && self.is_known_non_negative(cx, found_rhs) {
                // Knowing that both FoundLHS and FoundRHS are non-negative, and
                // knowing FoundLHS >u FoundRHS, we also know that FoundLHS >s
                // FoundRHS. Let us use this fact to prove that LHS and RHS are
                // non-negative.
                let minus_one = self.get_minus_one(cx, self.type_of(lhs));
                if self.is_implied_cond_operands(cx, IntPredicate::Sgt, lhs, minus_one, found_lhs, found_rhs, None)
                    && self.is_implied_cond_operands(cx, IntPredicate::Sgt, rhs, minus_one, found_lhs, found_rhs, None)
                {
                    p = IntPredicate::Sgt;
                }
            }
        }

        if p != IntPredicate::Sgt {
            return false;
        }

        let get_op_from_sext = |se: &ScalarEvolution, s: Scev| match se.node(s).kind {
            Kind::SignExtend(operand) => operand.scev,
            // TODO (LLVM): If S is a SCEVConstant then you can cheaply "strip"
            // the sext off the constant in some cases.
            _ => s,
        };

        // Acquire values from extensions.
        let orig_lhs = lhs;
        let orig_found_lhs = found_lhs;
        lhs = get_op_from_sext(self, lhs);
        found_lhs = get_op_from_sext(self, found_lhs);

        // Is the SGT predicate can be proved trivially or using the found
        // context.
        let is_sgt_via_context = |se: &mut ScalarEvolution, s1: Scev, s2: Scev| {
            se.is_known_via_non_recursive_reasoning(cx, IntPredicate::Sgt, s1, s2)
                || se.is_implied_via_operations(cx, IntPredicate::Sgt, s1, s2, orig_found_lhs, found_rhs, depth + 1)
        };

        if let Kind::Add(operands) = self.node(lhs).kind.clone() {
            // We want to avoid creation of any new non-constant SCEV. Since we
            // are going to compare the operands to RHS, we should be certain
            // that we don't need any size extensions for this. So let's decline
            // all cases when the sizes of types of LHS and RHS do not match.
            // TODO (LLVM): Maybe try to get RHS from sext to catch more cases?
            if self.get_type_size_in_bits(cx, self.type_of(lhs)) != self.get_type_size_in_bits(cx, self.type_of(rhs)) {
                return false;
            }

            // Should not overflow.
            if self.get_no_wrap_flags(lhs, flags::NSW) == 0 {
                return false;
            }

            let ll = operands[0].scev;
            let lr = operands[1].scev;
            let minus_one = self.get_minus_one(cx, self.type_of(rhs));

            // Checks that S1 >= 0 && S2 > RHS, trivially or using the found
            // context.
            let is_sum_greater_than_rhs = |se: &mut ScalarEvolution, s1: Scev, s2: Scev| {
                is_sgt_via_context(se, s1, minus_one) && is_sgt_via_context(se, s2, rhs)
            };
            // Try to prove the following rule:
            // (LHS = LL + LR) && (LL >= 0) && (LR > RHS) => (LHS > RHS).
            // (LHS = LL + LR) && (LR >= 0) && (LL > RHS) => (LHS > RHS).
            if is_sum_greater_than_rhs(self, ll, lr) || is_sum_greater_than_rhs(self, lr, ll) {
                return true;
            }
        } else if let Some(value) = self.unknown_value(lhs) {
            let query = cx.query(None);
            if let Some((_, divide)) = query.instruction_of(value)
                && divide.opcode == Opcode::Binary(llrm_mir::opcode::BinaryOp::SDiv)
            {
                let (ll, lr) = (divide.operands[0], divide.operands[1]);
                // Rules for division.
                // We are going to perform some comparisons with Denominator and
                // its derivative expressions. In general case, creating a SCEV
                // for it may lead to a complex analysis of the entire graph,
                // and in particular it can request trip count recalculation for
                // the same loop. This would cache as SCEVCouldNotCompute to
                // avoid the infinite recursion. To avoid this, we only want to
                // create SCEVs that are constants in this section. So we bail
                // if Denominator is not a constant.
                if !matches!(lr, Operand::Constant(_)) {
                    return false;
                }

                let denominator = self.get_scev(cx, lr);
                if self.as_constant(denominator).is_none() {
                    return false;
                }

                // We want to make sure that LHS = FoundLHS / Denominator. If
                // it is so, then a SCEV for the numerator already exists and
                // matches with FoundLHS.
                let Some(numerator) = self.get_existing_scev(ll) else { return false };
                if self.type_of(numerator) != self.type_of(found_lhs) {
                    return false;
                }

                // Make sure that the numerator matches with FoundLHS and the
                // denominator is positive.
                if !self.has_same_value(cx, numerator, found_lhs) || !self.is_known_positive(cx, denominator) {
                    return false;
                }

                let dty = self.type_of(denominator);
                let frhs_ty = self.type_of(found_rhs);
                if dty.is_pointer() != frhs_ty.is_pointer() {
                    // One of types is a pointer and another one is not. We
                    // cannot extend them properly to a wider type, so let us
                    // just reject this case.
                    return false;
                }

                // Given that:
                // FoundLHS > FoundRHS, LHS = FoundLHS / Denominator,
                // Denominator > 0.
                let wty = self.get_wider_type(cx, dty, frhs_ty);
                let denominator_ext = self.get_noop_or_sign_extend(cx, denominator, wty);
                let found_rhs_ext = self.get_noop_or_sign_extend(cx, found_rhs, wty);

                // Try to prove the following rule:
                // (FoundRHS > Denominator - 2) && (RHS <= 0) => (LHS > RHS).
                // For example, given that FoundLHS > 2. It means that FoundLHS
                // is at least 3. If we divide it by Denominator < 4, we will
                // have at least 1.
                let two = self.get_constant_of_type(cx, wty, 2);
                let denom_minus_two = self.get_minus_scev(cx, Use::of(denominator_ext), Use::of(two), 0, 0);
                if self.is_known_non_positive(cx, rhs) && is_sgt_via_context(self, found_rhs_ext, denom_minus_two) {
                    return true;
                }

                // Try to prove the following rule:
                // (FoundRHS > -1 - Denominator) && (RHS < 0) => (LHS > RHS).
                // For example, given that FoundLHS > -3. Then FoundLHS is at
                // least -2. If we divide it by Denominator > 2, then:
                // 1. If FoundLHS is negative, then the result is 0.
                // 2. If FoundLHS is non-negative, then the result is
                //    non-negative.
                // Anyways, the result is non-negative.
                let minus_one = self.get_minus_one(cx, wty);
                let neg_denom_minus_one = self.get_minus_scev(cx, Use::of(minus_one), Use::of(denominator_ext), 0, 0);
                if self.is_known_negative(cx, rhs) && is_sgt_via_context(self, found_rhs_ext, neg_denom_minus_one) {
                    return true;
                }
            }
        }

        // If our expression contained SCEVUnknown Phis, and we split it down
        // and now need to prove something for them, try to prove the predicate
        // for every possible incoming values of those Phis.
        self.is_implied_via_merge(cx, pred, orig_lhs, rhs, orig_found_lhs, found_rhs, depth + 1)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12672
    /// `isImpliedViaMerge`.
    #[allow(clippy::too_many_arguments)]
    fn is_implied_via_merge(
        &mut self,
        cx: &Cx,
        mut pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
        mut found_lhs: Scev,
        mut found_rhs: Scev,
        depth: u32,
    ) -> bool {
        let query = cx.query(None);
        let phi_of = |se: &ScalarEvolution, s: Scev| -> Option<Operand> {
            let value = se.unknown_value(s)?;
            let (_, instruction) = query.instruction_of(value)?;
            (instruction.opcode == Opcode::Phi).then_some(value)
        };

        // Find respective Phis and check that they are not being pending.
        let mut l_phi = None;
        let mut r_phi = None;
        if let Some(phi) = phi_of(self, lhs) {
            if self.guards.pending_merges.contains(&phi) {
                return false;
            }
            self.guards.pending_merges.push(phi);
            l_phi = Some(phi);
        }
        if let Some(phi) = phi_of(self, rhs) {
            // If we detect a loop of Phi nodes being processed by this method,
            // we don't want to deal with a case that complex, so return
            // conservative answer false.
            if self.guards.pending_merges.contains(&phi) {
                if let Some(phi) = l_phi {
                    self.guards.pending_merges.retain(|one| *one != phi);
                }
                return false;
            }
            self.guards.pending_merges.push(phi);
            r_phi = Some(phi);
        }

        let proved = 'merge: {
            // If none of LHS, RHS is a Phi, nothing to do here.
            if l_phi.is_none() && r_phi.is_none() {
                break 'merge false;
            }

            // If there is a SCEVUnknown Phi we are interested in, make it left.
            if l_phi.is_none() {
                std::mem::swap(&mut lhs, &mut rhs);
                std::mem::swap(&mut found_lhs, &mut found_rhs);
                std::mem::swap(&mut l_phi, &mut r_phi);
                pred = pred.swapped();
            }

            let l_phi_value = l_phi.expect("LPhi should definitely be a SCEVUnknown Phi!");
            let (_, l_phi_instruction) = query.instruction_of(l_phi_value).expect("a phi");
            let l_phi_inst = query.instruction_of(l_phi_value).map(|(inst, _)| inst).expect("a phi");
            let lbb = cx.function.parent(l_phi_inst).expect("a placed phi");
            let rar = self.is_add_rec(rhs).then_some(rhs);

            let incoming = |instruction: &llrm_mir::module::Instruction, from: BlockId| -> Option<Operand> {
                pattern::phi_incoming(instruction).find(|&(_, block)| block == from).map(|(value, _)| value)
            };

            if let Some(r_phi_value) = r_phi
                && let Some((r_phi_inst, r_phi_instruction)) = query.instruction_of(r_phi_value)
                && cx.function.parent(r_phi_inst) == Some(lbb)
            {
                // Case one: RHS is also a SCEVUnknown Phi from the same basic
                // block. If we compare two Phis from the same block, and for
                // each entry block the predicate is true for incoming values
                // from this block, then the predicate is also true for the
                // Phis.
                for inc_bb in cx.function.predecessors(lbb).iter().copied().collect::<Vec<_>>() {
                    let (Some(lv), Some(rv)) =
                        (incoming(l_phi_instruction, inc_bb), incoming(r_phi_instruction, inc_bb))
                    else {
                        break 'merge false;
                    };
                    let (l, r) = (self.get_scev(cx, lv), self.get_scev(cx, rv));
                    if !self.proved_easily(cx, pred, l, r, found_lhs, found_rhs, depth) {
                        break 'merge false;
                    }
                }
            } else if let Some(rar) = rar
                && self.addrec_header(rar) == Some(lbb)
            {
                // Case two: RHS is also a Phi from the same basic block, and
                // it is an AddRec. It means that there is a loop which has both
                // AddRec and Unknown PHIs, for it we can compare incoming
                // values of AddRec from above the loop and latch with their
                // respective incoming values of LPhi.
                // TODO (LLVM): Generalize to handle loops with many inputs in a
                // header.
                if l_phi_instruction.operands.len() != 4 {
                    break 'merge false;
                }

                let r_loop = cx.loops.get_loop_for(lbb).expect("the loop of the AddRec");
                let predecessor =
                    r_loop.get_loop_predecessor(cx.function).expect("Loop with AddRec with no predecessor?");
                let Some(value) = incoming(l_phi_instruction, predecessor) else { break 'merge false };
                let l1 = self.get_scev(cx, value);
                let start = self.addrec_start(rar);
                if !self.proved_easily(cx, pred, l1, start, found_lhs, found_rhs, depth) {
                    break 'merge false;
                }
                let latch = r_loop.get_loop_latch(cx.function).expect("Loop with AddRec with no latch?");
                let Some(value) = incoming(l_phi_instruction, latch) else { break 'merge false };
                let l2 = self.get_scev(cx, value);
                let post_inc = self.get_post_inc_expr(cx, rar);
                if !self.proved_easily(cx, pred, l2, post_inc, found_lhs, found_rhs, depth) {
                    break 'merge false;
                }
            } else {
                // In all other cases go over inputs of LHS and compare each of
                // them to RHS, the predicate is true for (LHS, RHS) if it is
                // true for all such pairs. At this point RHS is either a
                // non-Phi, or it is a Phi from some block different from LBB.
                for inc_bb in cx.function.predecessors(lbb).iter().copied().collect::<Vec<_>>() {
                    // Check that RHS is available in this block.
                    if !self.dominates_block(cx, rhs, inc_bb) {
                        break 'merge false;
                    }
                    let Some(value) = incoming(l_phi_instruction, inc_bb) else { break 'merge false };
                    let l = self.get_scev(cx, value);
                    // Make sure L does not refer to a value from a potentially
                    // previous iteration of a loop.
                    if !self.properly_dominates(cx, l, lbb) {
                        break 'merge false;
                    }
                    // Addrecs are considered to properly dominate their loop,
                    // so are missed by the previous check. Discard any values
                    // that have computable evolution in this loop.
                    if let Some(l_loop) = cx.loops.get_loop_for(lbb)
                        && self.has_computable_loop_evolution(cx, l, Some(l_loop.get_header()))
                    {
                        break 'merge false;
                    }
                    if !self.proved_easily(cx, pred, l, rhs, found_lhs, found_rhs, depth) {
                        break 'merge false;
                    }
                }
            }
            true
        };

        if let Some(phi) = l_phi {
            self.guards.pending_merges.retain(|one| *one != phi);
        }
        if let Some(phi) = r_phi {
            self.guards.pending_merges.retain(|one| *one != phi);
        }
        proved
    }

    /// `ProvedEasily` of `isImpliedViaMerge`.
    #[allow(clippy::too_many_arguments)]
    fn proved_easily(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        s1: Scev,
        s2: Scev,
        found_lhs: Scev,
        found_rhs: Scev,
        depth: u32,
    ) -> bool {
        self.is_known_via_non_recursive_reasoning(cx, pred, s1, s2)
            || self.is_implied_cond_operands_via_ranges(cx, pred, s1, s2, pred, found_lhs, found_rhs)
            || self.is_implied_via_operations(cx, pred, s1, s2, found_lhs, found_rhs, depth)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14040
    /// `SCEVAddRecExpr::getPostIncExpr`: `{A+B,+,B+C,+,...,+,N}` of
    /// `{A,+,B,+,C,...,+,N}`.
    pub fn get_post_inc_expr(
        &mut self,
        cx: &Cx,
        add_rec: Scev,
    ) -> Scev {
        let Kind::AddRec(operands, header) = self.node(add_rec).kind.clone() else { unreachable!("an add recurrence") };
        debug_assert!(operands.len() > 1, "AddRec with zero step?");
        // There is a temptation to just call getAddExpr(this,
        // getStepRecurrence(SE)), but in this case we cannot guarantee that
        // the value returned will be an AddRec because SCEV does not have a
        // fixed point where it stops simplification: it is legal to return
        // ({rec1} + {rec2}). For example, it may happen if we reach arithmetic
        // depth limit while simplifying. So we construct the returned value
        // explicitly.
        let mut ops = Vec::new();
        // If this is {A,+,B,+,C,...,+,N}, then its step is {B,+,C,+,...,+,N},
        // and (this + Step) is {A+B,+,B+C,+...,+,N}.
        for i in 0..operands.len() - 1 {
            let sum = self.get_add_expr(cx, vec![operands[i], operands[i + 1]], Flags::default(), 0).scev;
            ops.push(Use::of(sum));
        }
        // We know that the last operand is not a constant zero (otherwise it
        // would have been popped out earlier). This guarantees us that if the
        // result has the same last operand, then it will also not be popped
        // out, meaning that the returned value will be an AddRec.
        ops.push(operands[operands.len() - 1]);
        self.get_add_rec_expr(cx, ops, header, Flags::default()).scev
    }
}
