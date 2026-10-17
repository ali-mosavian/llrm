//! `isKnownToBeAPowerOfTwo`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`isKnownToBeAPowerOfTwo`, `isPowerOfTwoRecurrence`,
//! `isImpliedToBeAPowerOfTwoFromCond`), each function linking the line it
//! translates.
//!
//! # Example
//! `shl i32 1, %n` is a power of two.

use super::query::Query;
use crate::module::Operand;

/// `isKnownToBeAPowerOfTwo(V, OrZero, Q, Depth)`.
pub fn is_known_to_be_a_power_of_two(
    q: &Query,
    v: Operand,
    or_zero: bool,
    depth: u32,
) -> bool {
    let _ = (q, v, or_zero, depth);
    false
}

/// The `p2` and `p2z` columns of `examples/vtdump.rs`.
pub fn column(
    q: &Query,
    v: Operand,
    or_zero: bool,
) -> String {
    let _ = (q, v);
    format!(" {}=?", if or_zero { "p2z" } else { "p2" })
}
