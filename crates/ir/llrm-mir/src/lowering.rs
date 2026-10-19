//! The target's lowering facts that passes translated from LLVM ask, as LLVM
//! splits them: `TargetLowering` answers these, `TargetTransformInfo`
//! (`tti`) prices instructions.
//!
//! # Example
//! ```
//! use llrm_mir::lowering::Lowering;
//! struct Plain;
//! impl Lowering for Plain {}
//! assert!(!Plain.is_mask_and_cmp0_folding_beneficial());
//! assert!(!Plain.is_mask_and_cmp0_folding_beneficial_for(Some(0x100)));
//! ```
//!
//! # Runs
//! Unit tests of the targets that answer (`llrm-x86`'s `lowering`).

/// `TargetLowering`'s hooks. Each default is the base class's.
pub trait Lowering {
    /// `isMaskAndCmp0FoldingBeneficial` (TargetLowering.h): whether an `and`
    /// compared only with zero is better made again at each compare, for the
    /// one instruction the pair becomes.
    fn is_mask_and_cmp0_folding_beneficial(&self) -> bool {
        false
    }

    /// The same question about the mask the `and` applies (`None`: unknown),
    /// which AArch64 answers by: a single-bit mask is a `tbz`, so the pair is
    /// folded; any other is better made again at each compare. By default the
    /// mask is not asked.
    fn is_mask_and_cmp0_folding_beneficial_for(
        &self,
        mask: Option<u128>,
    ) -> bool {
        let _ = mask;
        self.is_mask_and_cmp0_folding_beneficial()
    }
}
