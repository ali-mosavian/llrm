//! Exact-width integer facts for source-neutral MIR analyses.
//!
//! Direct port of the arithmetic core of `qbopt.analysis.consts`: `Known`
//! and `masked`.  Propagation itself remains a separate port.

use std::fmt;

use num_bigint::BigInt;

/// The low `width` bytes of a value are `n`; nothing is known above them.
///
/// Direct port of `qbopt.analysis.consts:Known`.
#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct Known {
    pub n: BigInt,
    pub width: u32,
}

impl Known {
    pub(crate) fn new(n: impl Into<BigInt>, width: u32) -> Self {
        Self { n: n.into(), width }
    }
}

impl fmt::Debug for Known {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:#x}:{}", self.n, self.width)
    }
}

/// Python's `masked(n, width)`.
pub(crate) fn masked(n: &BigInt, width: u32) -> BigInt {
    n & mask(width)
}

fn mask(width: u32) -> BigInt {
    (BigInt::from(1_u8) << (width * 8)) - 1
}
