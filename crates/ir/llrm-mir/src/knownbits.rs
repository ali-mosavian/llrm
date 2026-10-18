//! `KnownBits`: which bits of an integer are proven zero and which one.
//!
//! The type of LLVM's
//! [`KnownBits.h`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Support/KnownBits.h)
//! at d1106deb71cc, integers of at most 128 bits. This module is the stub of
//! the translation: the fields and the two constructors every caller needs; the
//! operations (`KnownBits::add`, `shl`, `computeForAddSub`, ...) are translated
//! here by the ValueTracking port, one function per C++ function.
//!
//! # Used by
//! [`crate::valuetracking::compute_known_bits`].

use crate::context::mask;

/// `llvm::KnownBits`: `zero` and `one` are the bits proven zero and one; a bit
/// in both is a conflict (the value is poison or the code unreachable).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KnownBits {
    pub zero: u128,
    pub one: u128,
    pub width: u32,
}

impl KnownBits {
    /// `KnownBits(BitWidth)`: nothing is known.
    pub fn unknown(width: u32) -> Self {
        Self { zero: 0, one: 0, width }
    }

    /// `KnownBits::makeConstant`.
    pub fn constant(
        width: u32,
        value: u128,
    ) -> Self {
        let all = mask(width);
        Self { zero: !value & all, one: value & all, width }
    }

    /// `hasConflict`.
    pub fn has_conflict(&self) -> bool {
        self.zero & self.one != 0
    }

    /// `isConstant`: every bit is known.
    pub fn is_constant(&self) -> bool {
        (self.zero | self.one) & mask(self.width) == mask(self.width) && !self.has_conflict()
    }

    /// `isNonNegative`: the sign bit is known zero.
    pub fn is_non_negative(&self) -> bool {
        self.zero >> (self.width - 1) & 1 == 1
    }

    /// `isNegative`: the sign bit is known one.
    pub fn is_negative(&self) -> bool {
        self.one >> (self.width - 1) & 1 == 1
    }
}
