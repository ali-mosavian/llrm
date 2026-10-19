//! `getLoopInvariantPredicate` and
//! `getLoopInvariantExitCondDuringFirstIterations`: a loop-varying compare of a
//! recurrence with an invariant, replaced by the same compare of the
//! recurrence's start.

use llrm_mir::module::{BlockId, InstId};
use llrm_mir::opcode::IntPredicate;

use super::expr::{Kind, Scev, flags};
use super::predicate::MonotonicPredicateType;
use super::predicate::{is_relational, is_signed};
use super::{Cx, ScalarEvolution};

/// `ScalarEvolution::LoopInvariantPredicate`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopInvariantPredicate {
    pub pred: IntPredicate,
    pub lhs: Scev,
    pub rhs: Scev,
}

/// `ICmpInst::getFlippedSignednessPredicate`.
fn flipped_signedness(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Ult => IntPredicate::Slt,
        IntPredicate::Ule => IntPredicate::Sle,
        IntPredicate::Ugt => IntPredicate::Sgt,
        IntPredicate::Uge => IntPredicate::Sge,
        IntPredicate::Slt => IntPredicate::Ult,
        IntPredicate::Sle => IntPredicate::Ule,
        IntPredicate::Sgt => IntPredicate::Ugt,
        IntPredicate::Sge => IntPredicate::Uge,
        other => other,
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11559
    /// `getLoopInvariantPredicate`.
    pub fn get_loop_invariant_predicate(
        &mut self,
        cx: &Cx,
        mut pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
        l: BlockId,
        ctx_i: Option<InstId>,
    ) -> Option<LoopInvariantPredicate> {
        // If there is a loop-invariant, force it into the RHS, otherwise bail
        // out.
        if !self.is_loop_invariant(cx, rhs, Some(l)) {
            if !self.is_loop_invariant(cx, lhs, Some(l)) {
                return None;
            }
            std::mem::swap(&mut lhs, &mut rhs);
            pred = pred.swapped();
        }

        let ar_lhs = lhs;
        match self.node(ar_lhs).kind {
            Kind::AddRec(_, header) if header == l => {}
            _ => return None,
        }

        let monotonic = self.get_monotonic_predicate_type(cx, ar_lhs, pred)?;
        // If the predicate "ArLHS `Pred` RHS" monotonically increases from
        // false to true as the loop iterates, and the backedge is control
        // dependent on "ArLHS `Pred` RHS" == true then the predicate can be
        // replaced with its value on the first iteration of the loop (which is
        // loop invariant). A decreasing predicate is the same with true and
        // false swapped.
        let increasing = monotonic == MonotonicPredicateType::MonotonicallyIncreasing;
        let p = if increasing { pred } else { pred.inverse() };

        if self.is_loop_backedge_guarded_by_cond_pred(cx, l, p, lhs, rhs) {
            return Some(LoopInvariantPredicate { pred, lhs: self.addrec_start(ar_lhs), rhs });
        }

        let ctx_i = ctx_i?;
        // Try to prove via context.
        // TODO (LLVM): Support other cases.
        if matches!(pred, IntPredicate::Ule | IntPredicate::Ult) {
            debug_assert!(self.get_no_wrap_flags(ar_lhs, flags::NUW) != 0, "Is a requirement of monotonicity!");
            // Given preconditions
            // (1) ArLHS does not cross the border of positive and negative
            //     parts of range because of: positive step, nuw, nsw;
            // (2) ArLHS <s RHS
            // (3) RHS >=s 0
            // the loop variant ArLHS <u RHS is Start(ArLHS) <u RHS.
            let sign_flipped = flipped_signedness(pred);
            if self.get_no_wrap_flags(ar_lhs, flags::NSW) != 0 && self.is_affine(ar_lhs) {
                let step = self.get_step_recurrence(cx, ar_lhs);
                if self.is_known_positive(cx, step)
                    && self.is_known_non_negative(cx, rhs)
                    && self.is_known_predicate_at(cx, sign_flipped, ar_lhs, rhs, Some(ctx_i))
                {
                    return Some(LoopInvariantPredicate { pred, lhs: self.addrec_start(ar_lhs), rhs });
                }
            }
        }

        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11644
    /// `getLoopInvariantExitCondDuringFirstIterations`.
    pub fn get_loop_invariant_exit_cond_during_first_iterations(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        l: BlockId,
        ctx_i: Option<InstId>,
        max_iter: Scev,
    ) -> Option<LoopInvariantPredicate> {
        if let Some(lip) =
            self.get_loop_invariant_exit_cond_during_first_iterations_impl(cx, pred, lhs, rhs, l, ctx_i, max_iter)
        {
            return Some(lip);
        }
        if let Kind::UMin(ops) = self.node(max_iter).kind.clone() {
            // Number of iterations expressed as UMIN isn't always great for
            // expressing the value on the last iteration. If the
            // straightforward approach didn't work, try the following trick:
            // if the a predicate is invariant for X, it is also invariant for
            // umin(X, ...). So try to find something that works among
            // subexpressions of MaxIter expressed as umin.
            for op in ops {
                if let Some(lip) = self
                    .get_loop_invariant_exit_cond_during_first_iterations_impl(cx, pred, lhs, rhs, l, ctx_i, op.scev)
                {
                    return Some(lip);
                }
            }
        }
        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11664
    /// `getLoopInvariantExitCondDuringFirstIterationsImpl`. `dropSameSign` is
    /// not needed: a predicate here carries no `samesign`.
    fn get_loop_invariant_exit_cond_during_first_iterations_impl(
        &mut self,
        cx: &Cx,
        mut pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
        l: BlockId,
        ctx_i: Option<InstId>,
        max_iter: Scev,
    ) -> Option<LoopInvariantPredicate> {
        // Try to prove the following set of facts:
        // - The predicate is monotonic in the iteration space.
        // - If the check does not fail on the 1st iteration:
        //   - No overflow will happen during first MaxIter iterations;
        //   - It will not fail on the MaxIter'th iteration.
        // If the check does fail on the 1st iteration, we leave the loop and no
        // other checks matter.

        // If there is a loop-invariant, force it into the RHS, otherwise bail
        // out.
        if !self.is_loop_invariant(cx, rhs, Some(l)) {
            if !self.is_loop_invariant(cx, lhs, Some(l)) {
                return None;
            }
            std::mem::swap(&mut lhs, &mut rhs);
            pred = pred.swapped();
        }

        let ar = lhs;
        match self.node(ar).kind {
            Kind::AddRec(_, header) if header == l => {}
            _ => return None,
        }

        // The predicate must be relational (i.e. <, <=, >=, >).
        if !is_relational(pred) {
            return None;
        }

        // TODO (LLVM): Support steps other than +/- 1.
        let step = self.get_step_recurrence(cx, ar);
        let ty = self.type_of(step);
        let one = self.get_one(cx, ty);
        let minus_one = self.get_negative_scev(cx, one, flags::NONE);
        if step != one && step != minus_one {
            return None;
        }

        // Type mismatch here means that MaxIter is potentially larger than max
        // unsigned value in start type, which mean we cannot prove no wrap for
        // the indvar.
        if self.type_of(ar) != self.type_of(max_iter) {
            return None;
        }

        // Value of IV on suggested last iteration.
        let last = self.evaluate_at_iteration(cx, ar, max_iter);
        // Does it still meet the requirement?
        if !self.is_loop_backedge_guarded_by_cond_pred(cx, l, pred, last, rhs) {
            return None;
        }
        // Because step is +/- 1 and MaxIter has same type as Start (i.e. it
        // does not exceed max unsigned value of this type), this effectively
        // proves that there is no wrap during the iteration. To prove that
        // there is no signed/unsigned wrap, we need to check that Start <= Last
        // for step = 1 or Start >= Last for step = -1.
        let mut no_overflow = if is_signed(pred) { IntPredicate::Sle } else { IntPredicate::Ule };
        if step == minus_one {
            no_overflow = no_overflow.swapped();
        }
        let start = self.addrec_start(ar);
        if !self.is_known_predicate_at(cx, no_overflow, start, last, ctx_i) {
            return None;
        }

        // Everything is fine.
        Some(LoopInvariantPredicate { pred, lhs: start, rhs })
    }
}
