//! `TargetRegisterInfo`: the register-file costs the greedy allocator reads,
//! which differ by target. Translated from
//! llvm/lib/CodeGen/TargetRegisterInfo.h and llvm/lib/Target/X86/
//! X86RegisterInfo.cpp at d1106deb71cc.

use llrm_lir::registers::RegId;

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

    /// Whether each lane of a callee-saved register is saved on its own, so
    /// using one costs its own first use (X86 saves the whole register).
    fn lanes_saved_apart(&self) -> bool {
        false
    }

    /// DEVIATE CSR5: where code is sized in bytes, what a callee-saved
    /// register and a spill cost in them; none: the pin's frequency rule.
    fn size_bytes(&self) -> Option<&SizeBytes> {
        None
    }

    /// `RegCosts[phys]`, the register's `CostPerUse`.
    fn reg_cost(
        &self,
        _phys: RegId,
    ) -> u8 {
        0
    }
}

/// The bytes of the instructions a callee-saved register and a spill are made
/// of, from the target's sheet (`opcosts.txt`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SizeBytes {
    pub push: u32,
    pub pop: u32,
    /// A reload and a store of a spilled value.
    pub load: u32,
    pub store: u32,
    /// Making a value again from a constant, the most a rematerialized one
    /// costs at each use.
    pub constant: u32,
    /// What a register past the legacy eight adds to an instruction naming it.
    pub extended_prefix: u32,
}

impl SizeBytes {
    /// A push and a pop of `phys`.
    pub fn save_restore(
        &self,
        phys: RegId,
    ) -> u32 {
        let extended = u32::from(is_extended(phys));
        self.push + self.pop + 2 * extended * self.extended_prefix
    }
}

/// `r8`..`r15`: the registers a REX prefix names.
fn is_extended(phys: RegId) -> bool {
    let name = format!("{phys:?}").to_lowercase();
    name.strip_prefix('r').and_then(|n| n.parse::<u32>().ok()).is_some_and(|n| n >= 8)
}

/// `X86RegisterInfo` of a function sized in bytes (`optsize`).
pub struct X86SizeRegisterInfo(pub SizeBytes);

impl TargetRegisterInfo for X86SizeRegisterInfo {
    fn copy_cost(&self) -> u32 {
        X86RegisterInfo.copy_cost()
    }

    fn csr_first_use_cost(
        &self,
        any_saved: bool,
    ) -> u32 {
        X86RegisterInfo.csr_first_use_cost(any_saved)
    }

    fn size_bytes(&self) -> Option<&SizeBytes> {
        Some(&self.0)
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
