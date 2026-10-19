//! What the mid-end asks of the arm64 target (`llrm_mir::target::Machine`): its
//! register budget, address forms and what an operation costs. The prices are a
//! stand-in until `AArch64TargetTransformInfo` is translated
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp>):
//! the mid-end reads a `Tti` first where the machine has one.

use std::collections::BTreeSet;

use llrm_mir::spaces::Spaces;
use llrm_mir::target::{AddressForm, Machine, OperationCosts};

/// The OS and ABI arm64 code is built for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    /// Apple's arm64 (`arm64-apple-macos`): Mach-O, `x18` reserved, variadic
    /// arguments on the stack.
    Darwin,
    /// AAPCS64 on Linux (`aarch64-unknown-linux-gnu`): ELF.
    Linux,
}

impl Platform {
    /// The LLVM triple a module of this platform states.
    pub fn triple(self) -> &'static str {
        match self {
            Platform::Darwin => "arm64-apple-macosx",
            Platform::Linux => "aarch64-unknown-linux-gnu",
        }
    }

    /// The data layout clang states for the platform (the pin's).
    pub fn datalayout(self) -> &'static str {
        match self {
            Platform::Darwin => "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32",
            Platform::Linux => {
                "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
            }
        }
    }

    /// Whether a global's symbol is spelled with a leading underscore.
    pub fn underscore(self) -> bool {
        self == Platform::Darwin
    }
}

/// The arm64 cost model the passes of the mid-end read.
#[derive(Clone, Debug)]
pub struct Arm64Machine {
    pub platform: Platform,
}

impl Machine for Arm64Machine {
    fn foreign_span(
        &self,
        _selectors: (i64, i64),
        _offsets: (i64, i64),
        _width: i64,
    ) -> Option<(i64, i64)> {
        None
    }

    fn spaces(&self) -> Spaces {
        Spaces::FLAT
    }

    fn costs(&self) -> OperationCosts {
        OperationCosts::default()
    }

    /// `x0`..`x15`, `x19`..`x28`: the allocatable integer registers.
    fn registers(&self) -> i64 {
        26
    }

    /// `x19`..`x28`.
    fn call_registers(&self) -> i64 {
        10
    }

    fn float_registers(&self) -> i64 {
        32
    }

    /// Base plus index, scaled by the access size or not, the index a 64-bit
    /// register or an extended 32-bit one.
    fn address_forms(&self) -> Vec<AddressForm> {
        let scales: BTreeSet<i64> = [1, 2, 4, 8].into_iter().collect();
        vec![AddressForm::new(8, scales, 0, 0, 0, false, None).expect("a well-formed address form")]
    }
}
