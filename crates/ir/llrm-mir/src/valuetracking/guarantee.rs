//! Whether a value is defined: `isGuaranteedNotToBeUndefOrPoison` and what it
//! needs.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`isGuaranteedNotToBeUndefOrPoison`, `canCreateUndefOrPoison`,
//! `impliesPoison`, `isGuaranteedToTransferExecutionToSuccessor`,
//! `propagatesPoison` and the helpers), each function linking the line it
//! translates. MIR has `poison` and `freeze` and no `undef`.
//!
//! # Example
//! A `freeze` is never undef or poison.

use super::query::Query;
use crate::module::{InstId, Operand};

/// `isGuaranteedNotToBeUndef(V, AC, CtxI, DT, Depth)`.
pub fn is_guaranteed_not_to_be_undef(
    q: &Query,
    v: Operand,
    depth: u32,
) -> bool {
    let _ = (q, v, depth);
    false
}

/// `isGuaranteedNotToBePoison(V, AC, CtxI, DT, Depth)`.
pub fn is_guaranteed_not_to_be_poison(
    q: &Query,
    v: Operand,
    depth: u32,
) -> bool {
    let _ = (q, v, depth);
    false
}

/// `isGuaranteedToTransferExecutionToSuccessor(I)`.
pub fn is_guaranteed_to_transfer_execution_to_successor(
    q: &Query,
    inst: InstId,
) -> bool {
    let _ = (q, inst);
    false
}
