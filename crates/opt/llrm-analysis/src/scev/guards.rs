//! `ScalarEvolution::LoopGuards` and `applyLoopGuards`: the facts the guards
//! of a loop give about the expressions in it (S5). Signatures; the bodies are
//! `todo!()` until S5.

use llrm_mir::hash::HashMap;
use llrm_mir::module::BlockId;

use super::expr::Scev;
use super::{Cx, ScalarEvolution};

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1579
/// `ScalarEvolution::LoopGuards`.
#[derive(Clone, Debug, Default)]
pub struct LoopGuards {
    pub rewrite_map: HashMap<Scev, Scev>,
    pub not_equal: Vec<(Scev, Scev)>,
    pub preserve_nuw: bool,
    pub preserve_nsw: bool,
}

impl LoopGuards {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15749
    /// `LoopGuards::collect(L, SE)`.
    pub fn collect(
        _se: &mut ScalarEvolution,
        _cx: &Cx,
        _l: BlockId,
    ) -> Self {
        todo!("S5")
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L16296
    /// `LoopGuards::rewrite(Expr)`, as `applyLoopGuards(Expr, Guards)`.
    pub fn apply_loop_guards_with(
        &mut self,
        _cx: &Cx,
        _expr: Scev,
        _guards: &LoopGuards,
    ) -> Scev {
        todo!("S5")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.cpp#L11290
    /// `isKnownToBeAPowerOfTwo(S, OrZero, OrNegative)`.
    pub fn is_known_to_be_a_power_of_two(
        &mut self,
        _cx: &Cx,
        _s: Scev,
        _or_zero: bool,
        _or_negative: bool,
    ) -> bool {
        todo!("S5")
    }
}
