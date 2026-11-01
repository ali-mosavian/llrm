//! `TargetRegisterInfo`: the register-file costs the greedy allocator reads,
//! which differ by target. Translated from
//! llvm/lib/CodeGen/TargetRegisterInfo.h and llvm/lib/Target/X86/
//! X86RegisterInfo.cpp at d1106deb71cc.

/// What greedy asks of the target's registers beyond the description's file.
pub trait TargetRegisterInfo {
    /// `RC->getCopyCost()`: the cost of one copy in a class.
    fn copy_cost(&self) -> u32;

    /// `getCSRFirstUseCost`: what the first use of a callee-saved register
    /// adds to the function (its save and its restore). `any_saved` is whether
    /// the target has callee-saved registers at all.
    fn csr_first_use_cost(
        &self,
        any_saved: bool,
    ) -> u32;

    /// `getCSRCostScale`: the percentage of that cost the allocator charges
    /// (the default is LLVM's, X86 takes it).
    fn csr_cost_scale(&self) -> u32 {
        30
    }

    /// `GreedyEnv::csr_spill_first`.
    fn csr_spill_first(&self) -> bool {
        false
    }
}

/// `X86RegisterInfo`.
pub struct X86RegisterInfo;

impl TargetRegisterInfo for X86RegisterInfo {
    fn copy_cost(&self) -> u32 {
        1
    }

    /// push + pop (`X86RegisterInfo::getCSRFirstUseCost`).
    fn csr_first_use_cost(
        &self,
        any_saved: bool,
    ) -> u32 {
        if any_saved { 2 } else { 0 }
    }
}
