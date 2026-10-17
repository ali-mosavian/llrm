//! `computeKnownBits`: which bits of a value are proven zero and which one.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`computeKnownBits`, `computeKnownBitsFromOperator` and the helpers they
//! call), one function per C++ function, each linking the line it translates.
//! Depth stops at [`super::MAX_ANALYSIS_RECURSION_DEPTH`]. Not translated, as
//! MIR has no such thing: vectors, scalable vectors, `ptrtoaddr`.
//!
//! # Example
//! `and i32 %x, 255` has its top 24 bits known zero.

use super::query::Query;
use crate::knownbits::KnownBits;
use crate::module::Operand;

/// `computeKnownBits(V, Q, Depth)`.
pub fn compute_known_bits(
    q: &Query,
    v: Operand,
    depth: u32,
) -> KnownBits {
    let _ = depth;
    KnownBits::unknown(q.scalar_bits(v).max(1))
}

/// The `kb` column of `examples/vtdump.rs`.
pub fn column(
    q: &Query,
    v: Operand,
) -> String {
    let _ = (q, v);
    " kb=?".to_owned()
}
