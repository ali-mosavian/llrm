//! What the folds and the loop passes ask of `BackedgeTakenInfo`: the exit
//! and trip counts of a loop, named by its header. Signatures as at the pin;
//! the counts are found by `trip.rs`.

use llrm_mir::module::BlockId;

use super::expr::{Scev, Ty, Use};
use super::{Cx, Flags, ScalarEvolution};

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1062
/// `ScalarEvolution::ExitCountKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitCountKind {
    Exact,
    ConstantMaximum,
    SymbolicMaximum,
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8548
    /// `getBackedgeTakenCount(L, Kind)`, also
    /// `getConstantMaxBackedgeTakenCount` and
    /// `getSymbolicMaxBackedgeTakenCount`.
    pub fn get_backedge_taken_count(
        &mut self,
        cx: &Cx,
        l: BlockId,
        kind: ExitCountKind,
    ) -> Scev {
        match kind {
            ExitCountKind::Exact => {
                let info = self.get_backedge_taken_info(cx, l);
                self.info_exact(cx, l, &info)
            }
            ExitCountKind::ConstantMaximum => self.get_backedge_taken_info(cx, l).get_constant_max(self),
            ExitCountKind::SymbolicMaximum => self.info_symbolic_max(cx, l),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8511
    /// `getExitCount`: the backedge count if `exiting_block` is the exit
    /// taken.
    pub fn get_exit_count(
        &mut self,
        cx: &Cx,
        l: BlockId,
        exiting_block: BlockId,
        kind: ExitCountKind,
    ) -> Scev {
        let info = self.get_backedge_taken_info(cx, l);
        match kind {
            ExitCountKind::Exact => info.get_exact_for(self, exiting_block),
            ExitCountKind::SymbolicMaximum => info.get_symbolic_max_for(self, exiting_block),
            ExitCountKind::ConstantMaximum => info.get_constant_max_for(self, exiting_block),
        }
    }

    /// `getPredicatedExitCount(L, ExitingBlock, Predicates, Kind)`.
    pub fn get_predicated_exit_count(
        &mut self,
        cx: &Cx,
        l: BlockId,
        exiting_block: BlockId,
        predicates: &mut Vec<super::wrap_predicate::Predicate>,
        kind: ExitCountKind,
    ) -> Scev {
        let info = self.get_predicated_backedge_taken_info(cx, l);
        info.exit_count_with(self, exiting_block, Some(predicates), kind)
    }

    /// `getPredicatedBackedgeTakenCount(L, Preds)`.
    pub fn get_predicated_backedge_taken_count(
        &mut self,
        cx: &Cx,
        l: BlockId,
        preds: &mut Vec<super::wrap_predicate::Predicate>,
    ) -> Scev {
        let info = self.get_predicated_backedge_taken_info(cx, l);
        self.info_exact_with(cx, l, &info, Some(preds))
    }

    /// `getPredicatedSymbolicMaxBackedgeTakenCount(L, Preds)`.
    pub fn get_predicated_symbolic_max_backedge_taken_count(
        &mut self,
        cx: &Cx,
        l: BlockId,
        preds: &mut Vec<super::wrap_predicate::Predicate>,
    ) -> Scev {
        let info = self.get_predicated_backedge_taken_info(cx, l);
        self.info_symbolic_max_with(cx, &info, preds)
    }

    /// `getPredicatedConstantMaxBackedgeTakenCount(L, Preds)`.
    pub fn get_predicated_constant_max_backedge_taken_count(
        &mut self,
        cx: &Cx,
        l: BlockId,
        preds: &mut Vec<super::wrap_predicate::Predicate>,
    ) -> Scev {
        let info = self.get_predicated_backedge_taken_info(cx, l);
        info.constant_max_with(self, preds)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1145
    /// `hasLoopInvariantBackedgeTakenCount`.
    pub fn has_loop_invariant_backedge_taken_count(
        &mut self,
        cx: &Cx,
        l: BlockId,
    ) -> bool {
        let count = self.get_backedge_taken_count(cx, l, ExitCountKind::Exact);
        count != self.get_could_not_compute()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8574
    /// `isBackedgeTakenCountMaxOrZero`.
    pub fn is_backedge_taken_count_max_or_zero(
        &mut self,
        cx: &Cx,
        l: BlockId,
    ) -> bool {
        self.get_backedge_taken_info(cx, l).is_constant_max_or_zero()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8359
    /// `getTripCountFromExitCount(ExitCount, EvalTy, L)`.
    pub fn get_trip_count_from_exit_count(
        &mut self,
        cx: &Cx,
        exit_count: Scev,
        eval_ty: Ty,
        l: Option<BlockId>,
    ) -> Scev {
        if exit_count == self.get_could_not_compute() {
            return self.get_could_not_compute();
        }

        let exit_count_size = self.get_type_size_in_bits(cx, self.type_of(exit_count));
        let eval_size = self.get_type_size_in_bits(cx, eval_ty);

        let can_add_one_without_overflow = |se: &mut ScalarEvolution| {
            let range = se.get_unsigned_range(cx, exit_count);
            if !range.contains(super::ap::Ap::all_ones(exit_count_size).bits) {
                return true;
            }

            let minus_one = se.get_minus_one(cx, se.type_of(exit_count));
            l.is_some_and(|l| {
                se.is_loop_entry_guarded_by_cond_pred(cx, l, llrm_mir::opcode::IntPredicate::Ne, exit_count, minus_one)
            })
        };

        // If we need to zero extend the backedge count, check if we can add
        // one to it prior to zero extending without overflow. Provided this is
        // safe, it allows better simplification of the +1.
        if eval_size > exit_count_size && can_add_one_without_overflow(self) {
            let one = self.get_one(cx, self.type_of(exit_count));
            let sum = self.get_add_expr(cx, vec![Use::of(exit_count), Use::of(one)], Flags::default(), 0).scev;
            return self.get_zero_extend_expr(cx, Use::of(sum), eval_ty, 0);
        }

        // Get the total trip count from the count by adding 1. This may wrap.
        let widened = self.get_truncate_or_zero_extend(cx, exit_count, eval_ty, 0);
        let one = self.get_one(cx, eval_ty);
        self.get_add_expr(cx, vec![Use::of(widened), Use::of(one)], Flags::default(), 0).scev
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8348
    /// `getTripCountFromExitCount(ExitCount)`: an evaluation type that cannot
    /// overflow.
    pub fn get_trip_count_from_exit_count_in_type(
        &mut self,
        cx: &Cx,
        exit_count: Scev,
    ) -> Scev {
        if exit_count == self.get_could_not_compute() {
            return self.get_could_not_compute();
        }

        let Ty::Int(bits) = self.type_of(exit_count) else { unreachable!("an integer exit count") };
        self.get_trip_count_from_exit_count(cx, exit_count, Ty::Int(bits + 1), None)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8389
    /// `getConstantTripCount`: the exit count as a trip count, 0 where it is
    /// not a constant under 2^32.
    fn constant_trip_count(
        &self,
        exit_count: Scev,
    ) -> u32 {
        let Some(exit_const) = self.as_constant(exit_count) else { return 0 };

        // Guard against huge trip counts.
        if exit_const.active_bits() > 32 {
            return 0;
        }

        // In case of integer overflow, this returns 0, which is correct.
        (exit_const.zext_value() as u32).wrapping_add(1)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8403
    /// `getSmallConstantTripCount(L)`: 0 is unknown.
    pub fn get_small_constant_trip_count(
        &mut self,
        cx: &Cx,
        l: BlockId,
    ) -> u32 {
        let exit_count = self.get_backedge_taken_count(cx, l, ExitCountKind::Exact);
        self.constant_trip_count(exit_count)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8410
    /// `getSmallConstantTripCount(L, ExitingBlock)`.
    pub fn get_small_constant_trip_count_for_exit(
        &mut self,
        cx: &Cx,
        l: BlockId,
        exiting_block: BlockId,
    ) -> u32 {
        let exit_count = self.get_exit_count(cx, l, exiting_block, ExitCountKind::Exact);
        self.constant_trip_count(exit_count)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8419
    /// `getSmallConstantMaxTripCount(L)` with no predicates.
    pub fn get_small_constant_max_trip_count(
        &mut self,
        cx: &Cx,
        l: BlockId,
    ) -> u32 {
        let max_exit_count = self.get_backedge_taken_count(cx, l, ExitCountKind::ConstantMaximum);
        self.constant_trip_count(max_exit_count)
    }

    /// `getSmallConstantMaxTripCount(L, &Predicates)`: the count under the
    /// predicates appended to `predicates`.
    pub fn get_small_constant_max_trip_count_with_predicates(
        &mut self,
        cx: &Cx,
        l: BlockId,
        predicates: &mut Vec<super::wrap_predicate::Predicate>,
    ) -> u32 {
        let max_exit_count = self.get_predicated_constant_max_backedge_taken_count(cx, l, predicates);
        self.constant_trip_count(max_exit_count)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8428
    /// `getSmallConstantTripMultiple(L)`: 1 is unknown.
    pub fn get_small_constant_trip_multiple(
        &mut self,
        cx: &Cx,
        l: BlockId,
    ) -> u32 {
        let loop_ = cx.loops.get_loop_for(l).expect("the header of a loop");
        let exiting_blocks = loop_.get_exiting_blocks(cx.function);

        // An exit with an uncomputable exit count makes the result 1.
        if exiting_blocks.is_empty()
            || exiting_blocks.iter().any(|&exiting| {
                self.get_exit_count(cx, l, exiting, ExitCountKind::Exact) == self.get_could_not_compute()
            })
        {
            return 1;
        }

        let mut res = 0;
        for &exiting in &exiting_blocks {
            let exit_count = self.get_exit_count(cx, l, exiting, ExitCountKind::Exact);
            res = gcd(res, self.trip_multiple_of(cx, l, exit_count));
        }
        res
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8444
    /// `getSmallConstantTripMultiple(ExitCount, Guards)`, the guards being
    /// `LoopGuards::collect(L)`.
    fn trip_multiple_of(
        &mut self,
        cx: &Cx,
        l: BlockId,
        exit_count: Scev,
    ) -> u32 {
        debug_assert!(exit_count != self.get_could_not_compute(), "Must be computable!");

        // Get the trip count
        let guarded = self.apply_loop_guards(cx, exit_count, l);
        let tc_expr = self.get_trip_count_from_exit_count_in_type(cx, guarded);

        let multiple = self.get_non_zero_constant_multiple(cx, tc_expr);
        // If a trip multiple is huge (>=2^32), the trip count is still
        // divisible by the greatest power of 2 divisor less than 2^32.
        if multiple.active_bits() > 32 { 1u32 << multiple.countr_zero().min(31) } else { multiple.zext_value() as u32 }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8464
    /// `getSmallConstantTripMultiple(L, ExitingBlock)`.
    pub fn get_small_constant_trip_multiple_for_exit(
        &mut self,
        cx: &Cx,
        l: BlockId,
        exiting_block: BlockId,
    ) -> u32 {
        let exit_count = self.get_exit_count(cx, l, exiting_block, ExitCountKind::Exact);
        if exit_count == self.get_could_not_compute() {
            return 1;
        }
        self.trip_multiple_of(cx, l, exit_count)
    }
}

fn gcd(
    a: u32,
    b: u32,
) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}
