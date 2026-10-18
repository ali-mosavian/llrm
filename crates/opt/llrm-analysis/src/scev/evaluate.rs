//! `evaluatePredicate` and `evaluatePredicateAt`: whether a comparison is
//! true, false or neither, with and without the guards of a block.

use llrm_mir::module::BlockId;
use llrm_mir::opcode::IntPredicate;

use super::expr::Scev;
use super::{Cx, ScalarEvolution};

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11454
    /// `evaluatePredicate`: `Some(true)` if `lhs pred rhs` is known,
    /// `Some(false)` if its inverse is.
    pub fn evaluate_predicate(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> Option<bool> {
        if self.is_known_predicate_scev(cx, pred, lhs, rhs) {
            return Some(true);
        }
        if self.is_known_predicate_scev(cx, pred.inverse(), lhs, rhs) {
            return Some(false);
        }
        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11473
    /// `evaluatePredicateAt`: as [`Self::evaluate_predicate`], and by the
    /// conditions that guard the entry of `ctx_block`, the block of the
    /// instruction `CtxI`.
    pub fn evaluate_predicate_at(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        ctx_block: BlockId,
    ) -> Option<bool> {
        let known_without_context = self.evaluate_predicate(cx, pred, lhs, rhs);
        if known_without_context.is_some() {
            return known_without_context;
        }

        if self.is_basic_block_entry_guarded_by_cond(cx, ctx_block, pred, lhs, rhs) {
            return Some(true);
        }
        if self.is_basic_block_entry_guarded_by_cond(cx, ctx_block, pred.inverse(), lhs, rhs) {
            return Some(false);
        }
        None
    }
}
