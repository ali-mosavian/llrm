//! `ConstantRange`: the integers from `lower` up to, not including, `upper`,
//! wrapping at the width.
//!
//! The type of LLVM's
//! [`ConstantRange.h`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/IR/ConstantRange.h)
//! at d1106deb71cc, integers of at most 128 bits. This module is the stub of
//! the translation: the representation and the two sets that need no operation;
//! the operations (`makeExactICmpRegion`, `icmp`, `add`, `contains`, `inverse`,
//! ...) are translated here by the ValueTracking port, one function per C++
//! function.
//!
//! # Used by
//! [`crate::valuetracking::compute_constant_range`].

use crate::context::mask;

/// `llvm::ConstantRange`: `[lower, upper)` modulo `2^width`; `lower == upper`
/// is the full set when it is the maximum value and the empty set when it is
/// zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstantRange {
    pub lower: u128,
    pub upper: u128,
    pub width: u32,
}

impl ConstantRange {
    /// `ConstantRange::getFull`.
    pub fn full(width: u32) -> Self {
        Self { lower: mask(width), upper: mask(width), width }
    }

    /// `ConstantRange::getEmpty`.
    pub fn empty(width: u32) -> Self {
        Self { lower: 0, upper: 0, width }
    }

    /// `isFullSet`.
    pub fn is_full_set(&self) -> bool {
        self.lower == self.upper && self.upper == mask(self.width)
    }

    /// `isEmptySet`.
    pub fn is_empty_set(&self) -> bool {
        self.lower == self.upper && self.upper == 0
    }
}
