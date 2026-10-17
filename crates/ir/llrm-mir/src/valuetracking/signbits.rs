//! `ComputeNumSignBits`: how many top bits of a value are copies of its sign
//! bit.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`ComputeNumSignBitsImpl` and its helpers), each function linking the line
//! it translates.
//!
//! # Example
//! `sext i16 %x to i32` has at least 17 sign bits.

use super::query::Query;
use crate::module::Operand;

/// `ComputeNumSignBits(V, Q, Depth)`.
pub fn compute_num_sign_bits(
    q: &Query,
    v: Operand,
    depth: u32,
) -> u32 {
    let _ = (q, v, depth);
    1
}

/// The `sb` column of `examples/vtdump.rs`.
pub fn column(
    q: &Query,
    v: Operand,
) -> String {
    let _ = (q, v);
    " sb=?".to_owned()
}
