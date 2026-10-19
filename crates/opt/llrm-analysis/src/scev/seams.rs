//! What the folds ask of the parts of ScalarEvolution that come after them:
//! ranges and flag inference (S2), predicates and guards (S5), and trip
//! counts (indvars-decide-poc's `BackedgeTakenInfo`). Each is a conservative
//! answer until its stage lands; the stack that lands S1 lands them with it.
//! A seam never makes a fold unsound, only less eager than LLVM's.

use super::expr::Scev;
use super::{Cx, ScalarEvolution};

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11274
    /// `isKnownNonNegative`.
    pub fn is_known_non_negative(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> bool {
        !self.get_signed_range_min(cx, s).is_negative()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11266
    /// `isKnownNegative`.
    pub fn is_known_negative(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> bool {
        self.get_signed_range_max(cx, s).is_negative()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11270
    /// `isKnownPositive`.
    pub fn is_known_positive(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> bool {
        self.get_signed_range_min(cx, s).is_strictly_positive()
    }

    /// `!AC.assumptions().empty()`.
    pub fn has_assumptions(
        &self,
        cx: &Cx,
    ) -> bool {
        !cx.assumptions.assumes.is_empty()
    }

    /// The type of an IR type id, for the folds that build integer types.
    pub fn int_ty(
        &self,
        bits: u32,
    ) -> super::expr::Ty {
        super::expr::Ty::Int(bits)
    }
}

impl ScalarEvolution {}
