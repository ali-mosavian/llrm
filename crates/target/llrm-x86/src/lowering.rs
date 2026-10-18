//! X86's `TargetLowering` facts, the same for every x86 target whatever its
//! word size.
//!
//! # Example
//! ```
//! use llrm_mir::lowering::Lowering;
//! assert!(llrm_x86::lowering::X86.is_mask_and_cmp0_folding_beneficial());
//! ```
//!
//! # Runs
//! `cargo test -p llrm-x86 lowering`.

use llrm_mir::lowering::Lowering;

/// The x86 family's answers.
pub struct X86;

impl Lowering for X86 {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.cpp#L3807-L3810
    fn is_mask_and_cmp0_folding_beneficial(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `test` makes the pair one instruction on every x86 target; the base
    /// class says no.
    #[test]
    fn test_x86_folds_a_mask_and_compare_with_zero() {
        assert!(X86.is_mask_and_cmp0_folding_beneficial());
    }
}
