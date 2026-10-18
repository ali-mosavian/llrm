//! What the folds and the loop passes ask of `BackedgeTakenInfo`
//! (indvars-decide-poc): the exit and trip counts of a loop, named by its
//! header. Signatures as at the pin; the bodies are the conservative answer
//! until the trip-count stage lands, then they are `todo!()` where there is
//! no conservative one. A loop with no computable count is `CouldNotCompute`.

use llrm_mir::module::BlockId;

use super::expr::{Scev, Ty, Use};
use super::{Cx, ScalarEvolution};

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1062
/// `ScalarEvolution::ExitCountKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitCountKind {
    Exact,
    ConstantMaximum,
    SymbolicMaximum,
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1099
    /// `getBackedgeTakenCount(L, Kind)`, also
    /// `getConstantMaxBackedgeTakenCount`
    /// and `getSymbolicMaxBackedgeTakenCount`. SEAM (BackedgeCounts):
    /// `CouldNotCompute` until trip counts land.
    pub fn get_backedge_taken_count(
        &mut self,
        _cx: &Cx,
        _l: BlockId,
        _kind: ExitCountKind,
    ) -> Scev {
        self.get_could_not_compute()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1078
    /// `getExitCount`: the backedge count if `exiting_block` is the exit taken.
    pub fn get_exit_count(
        &mut self,
        _cx: &Cx,
        _l: BlockId,
        _exiting_block: BlockId,
        _kind: ExitCountKind,
    ) -> Scev {
        todo!("BackedgeCounts")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1145
    /// `hasLoopInvariantBackedgeTakenCount`.
    pub fn has_loop_invariant_backedge_taken_count(
        &mut self,
        _cx: &Cx,
        _l: BlockId,
    ) -> bool {
        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1007
    /// `getTripCountFromExitCount(ExitCount, EvalTy, L)`.
    pub fn get_trip_count_from_exit_count(
        &mut self,
        _cx: &Cx,
        _exit_count: Scev,
        _eval_ty: Ty,
        _l: BlockId,
    ) -> Scev {
        todo!("BackedgeCounts")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L998
    /// `getTripCountFromExitCount(ExitCount)`: the exit count's own type.
    pub fn get_trip_count_from_exit_count_in_type(
        &mut self,
        _cx: &Cx,
        _exit_count: Scev,
    ) -> Scev {
        todo!("BackedgeCounts")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1014
    /// `getSmallConstantTripCount(L)`: 0 is unknown.
    pub fn get_small_constant_trip_count(
        &mut self,
        _cx: &Cx,
        _l: BlockId,
    ) -> u32 {
        0
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1025
    /// `getSmallConstantTripCount(L, ExitingBlock)`.
    pub fn get_small_constant_trip_count_for_exit(
        &mut self,
        _cx: &Cx,
        _l: BlockId,
        _exiting_block: BlockId,
    ) -> u32 {
        0
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1032
    /// `getSmallConstantMaxTripCount(L)` (the predicates are DEVIATE 1).
    pub fn get_small_constant_max_trip_count(
        &mut self,
        _cx: &Cx,
        _l: BlockId,
    ) -> u32 {
        0
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1048
    /// `getSmallConstantTripMultiple(L)`: 1 is unknown.
    pub fn get_small_constant_trip_multiple(
        &mut self,
        _cx: &Cx,
        _l: BlockId,
    ) -> u32 {
        1
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L986
    /// `SCEVAddRecExpr::evaluateAtIteration(Operands, It, SE, UseFlags)`: the
    /// chain `{A,+,B,+,C,...}` at iteration `it`. Translated with the trip
    /// counts (`how_many.rs`).
    pub fn evaluate_at_iteration_ops(
        &mut self,
        _cx: &Cx,
        _operands: &[Use],
        _it: Scev,
        _use_flags: u8,
    ) -> Use {
        todo!("BackedgeCounts")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L981
    /// `SCEVAddRecExpr::evaluateAtIteration(It, SE)`.
    pub fn evaluate_at_iteration(
        &mut self,
        _cx: &Cx,
        _add_rec: Scev,
        _it: Scev,
    ) -> Scev {
        todo!("BackedgeCounts")
    }
}
