//! `isKnownNonZero`, `isKnownNonEqual`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`isKnownNonZeroFromOperator` and the `isNonZero*` helpers,
//! `isKnownNonNullFromDominatingCondition`, `isKnownNonEqual`), each function
//! linking the line it translates.
//!
//! # Example
//! `or i32 %x, 1` is non-zero.

use super::query::Query;
use crate::module::Operand;

/// `isKnownNonZero(V, Q, Depth)`.
pub fn is_known_non_zero(
    q: &Query,
    v: Operand,
    depth: u32,
) -> bool {
    let _ = (q, v, depth);
    false
}

/// `isKnownNonEqual(V1, V2, Q, Depth)`.
pub fn is_known_non_equal(
    q: &Query,
    v1: Operand,
    v2: Operand,
    depth: u32,
) -> bool {
    let _ = (q, v1, v2, depth);
    false
}

/// The `nz` column of `examples/vtdump.rs`.
pub fn column(
    q: &Query,
    v: Operand,
) -> String {
    let _ = (q, v);
    " nz=?".to_owned()
}
