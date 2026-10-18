//! `proveNoSignedWrapViaInduction`, `proveNoUnsignedWrapViaInduction` and
//! `isKnownOnEveryIteration`: a recurrence does not wrap when its backedge is
//! guarded by a comparison of the recurrence with the limit its step leaves.

use llrm_mir::opcode::IntPredicate;

use super::expr::{Kind, Scev, flags};
use super::{Cx, ScalarEvolution};

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11487
    /// `isKnownOnEveryIteration`: `lhs pred rhs` at the entry of the loop of
    /// the recurrence `lhs`, and of its post-increment on the backedge.
    pub fn is_known_on_every_iteration(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> bool {
        let Kind::AddRec(operands, l) = self.node(lhs).kind.clone() else { unreachable!("an add recurrence") };
        if !self.is_loop_entry_guarded_by_cond_pred(cx, l, pred, operands[0].scev, rhs) {
            return false;
        }
        let post_inc = self.get_post_inc_expr(cx, lhs);
        self.is_loop_backedge_guarded_by_cond_pred(cx, l, pred, post_inc, rhs)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5089
    /// `proveNoSignedWrapViaInduction`: the no-wrap flags of `ar` with `NSW`
    /// if it is proved.
    pub fn prove_no_signed_wrap_via_induction(
        &mut self,
        cx: &Cx,
        ar: Scev,
    ) -> u8 {
        let mut result = self.get_no_wrap_flags(ar, flags::MASK);

        if self.get_no_wrap_flags(ar, flags::NSW) != 0 {
            return result;
        }

        if !self.is_affine(ar) {
            return result;
        }

        // This function can be expensive, only try to prove NSW once per
        // AddRec.
        if !self.signed_wrap_via_induction_tried.insert(ar) {
            return result;
        }

        let Kind::AddRec(_, l) = self.node(ar).kind else { unreachable!("an add recurrence") };
        let step = self.get_step_recurrence(cx, ar);

        // Check whether the backedge-taken count is SCEVCouldNotCompute. Note
        // that this serves two purposes: It filters out loops that are simply
        // not analyzable, and it covers the case where this code is being
        // called from within backedge-taken count analysis, such that
        // attempting to ask for the backedge-taken count would likely result
        // in infinite recursion. In the later case, the analysis code will
        // cope with a conservative value, and it will take care to purge that
        // value once it has finished.
        let max_be_count = self.get_backedge_taken_count(cx, l, super::backedge::ExitCountKind::ConstantMaximum);

        // Normally, in the cases we can prove no-overflow via a backedge
        // guarding condition, we can also compute a backedge taken count for
        // the loop. The exceptions are assumptions and guards present in the
        // loop -- SCEV is not great at exploiting these to compute max
        // backedge taken counts, but can still use these to prove lack of
        // overflow. Use this fact to avoid doing extra work that may not pay
        // off.
        if matches!(self.node(max_be_count).kind, Kind::CouldNotCompute)
            && !self.has_guards(cx)
            && !self.has_assumptions(cx)
        {
            return result;
        }

        // If the backedge is guarded by a comparison with the pre-inc value
        // the addrec is safe. Also, if the entry is guarded by a comparison
        // with the start value and the backedge is guarded by a comparison
        // with the post-inc value, the addrec is safe.
        if let Some((pred, overflow_limit)) = self.get_signed_overflow_limit_for_step(cx, step)
            && (self.is_loop_backedge_guarded_by_cond_pred(cx, l, pred, ar, overflow_limit)
                || self.is_known_on_every_iteration(cx, pred, ar, overflow_limit))
        {
            result |= flags::NSW;
        }
        result
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5142
    /// `proveNoUnsignedWrapViaInduction`.
    pub fn prove_no_unsigned_wrap_via_induction(
        &mut self,
        cx: &Cx,
        ar: Scev,
    ) -> u8 {
        let mut result = self.get_no_wrap_flags(ar, flags::MASK);

        if self.get_no_wrap_flags(ar, flags::NUW) != 0 {
            return result;
        }

        if !self.is_affine(ar) {
            return result;
        }

        // This function can be expensive, only try to prove NUW once per
        // AddRec.
        if !self.unsigned_wrap_via_induction_tried.insert(ar) {
            return result;
        }

        let Kind::AddRec(_, l) = self.node(ar).kind else { unreachable!("an add recurrence") };
        let step = self.get_step_recurrence(cx, ar);

        // See the signed case for why the backedge-taken count is asked.
        let max_be_count = self.get_backedge_taken_count(cx, l, super::backedge::ExitCountKind::ConstantMaximum);

        if matches!(self.node(max_be_count).kind, Kind::CouldNotCompute)
            && !self.has_guards(cx)
            && !self.has_assumptions(cx)
        {
            return result;
        }

        // If the backedge is guarded by a comparison with the pre-inc value
        // the addrec is safe. Also, if the entry is guarded by a comparison
        // with the start value and the backedge is guarded by a comparison
        // with the post-inc value, the addrec is safe.
        if self.is_known_positive(cx, step) {
            let (pred, overflow_limit) = self.get_unsigned_overflow_limit_for_step(cx, step);
            if self.is_loop_backedge_guarded_by_cond_pred(cx, l, pred, ar, overflow_limit)
                || self.is_known_on_every_iteration(cx, pred, ar, overflow_limit)
            {
                result |= flags::NUW;
            }
        }
        result
    }
}
