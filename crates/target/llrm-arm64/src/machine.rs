//! What the mid-end asks of the arm64 target (`llrm_mir::target::Machine`): its
//! register budget, address forms and what an operation costs. The mid-end
//! reads the `Tti` (`AArch64TargetTransformInfo`, translated in `tti`) first;
//! the prices here are the fallback for what it does not answer.

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::lowering::Lowering;
use llrm_mir::spaces::Spaces;
use llrm_mir::target::{AddressForm, Machine, OperationCosts};
use llrm_mir::tti::Tti;

use crate::tti::{AArch64, AArch64Lowering};

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

    /// The platform's data layout description (`machines/datalayout*.toml`).
    pub fn layout_description(self) -> &'static str {
        match self {
            Platform::Darwin => include_str!("machines/datalayout-darwin.toml"),
            Platform::Linux => include_str!("machines/datalayout.toml"),
        }
    }

    /// The platform's object format description (`machines/object*.toml`).
    pub fn object_description(self) -> &'static str {
        match self {
            Platform::Darwin => include_str!("machines/object-darwin.toml"),
            Platform::Linux => include_str!("machines/object.toml"),
        }
    }

    /// The `-mcpu` the platform's cost model is for: Apple's M1 on Darwin, the
    /// generic cpu elsewhere.
    pub fn cpu(self) -> &'static str {
        match self {
            Platform::Darwin => "apple-m1",
            Platform::Linux => "generic",
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

    fn tti(&self) -> Option<Rc<dyn Tti>> {
        Some(Rc::new(AArch64::of_cpu(self.platform.cpu()).expect("a cpu the platform names")))
    }

    fn lowering(&self) -> Option<Rc<dyn Lowering>> {
        Some(Rc::new(AArch64Lowering))
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
