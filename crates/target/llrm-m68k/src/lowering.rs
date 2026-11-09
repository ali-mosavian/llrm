//! What CodeGenPrepare asks of m68k's `TargetLowering`.
//!
//! The pin's M68k registers `i8`, `i16` and `i32` as register types
//! ([M68kISelLowering.cpp#L62-L64](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kISelLowering.cpp#L62-L64))
//! and overrides none of the hooks the steps read, so every other answer is
//! the base class's ([`llrm_mir::lowering::Lowering`]'s defaults).
//!
//! The pin runs every step. A step is listed in [`runs`] once it is measured
//! not worse on the corpus (68000 and 68020, O1/O2/Os).

use llrm_mir::lowering::{CgpStep, Lowering};

/// m68k's answers to the CodeGenPrepare steps.
#[derive(Clone, Copy, Debug, Default)]
pub struct M68kLowering;

/// Whether `step` runs on m68k.
///
/// DEVIATE M68K-CGP-STAGED (owner m68k, exit 2026-11-30): the steps not
/// listed do not run yet. `OverflowMath` is worse on 18 corpus cells
/// (+186 B at 68000: the carry of an overflow node feeding a branch is
/// materialised); the others change nothing and join in a later commit.
pub fn runs(step: CgpStep) -> bool {
    match step {
        CgpStep::Branches
        | CgpStep::SwitchType
        | CgpStep::OverflowMath
        | CgpStep::Compares
        | CgpStep::MaskSinking
        | CgpStep::CastSinking
        | CgpStep::ExtPromotion
        | CgpStep::SplitStores => false,
    }
}

impl Lowering for M68kLowering {
    fn runs_cgp_step(
        &self,
        step: CgpStep,
    ) -> bool {
        runs(step)
    }

    fn is_type_legal(
        &self,
        bits: u32,
    ) -> bool {
        matches!(bits, 8 | 16 | 32)
    }
}
