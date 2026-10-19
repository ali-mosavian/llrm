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
}
