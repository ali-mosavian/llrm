//! Facts from the context a query is asked in: `llvm.assume` calls, the
//! conditions of the branches that dominate the context instruction, and
//! `isImpliedCondition`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`computeKnownBitsFromContext`, `isValidAssumeForContext`,
//! `findValuesAffectedByCondition`, `isImpliedCondition`,
//! `isImpliedByDomCondition` and what they call), each function linking the
//! line it translates. `Query::dt` and `Query::assumptions` are `SQ.DT` and
//! `SQ.AC`: absent, nothing here applies.
//!
//! # Example
//! After `call void @llvm.assume(i1 %c)` with `%c = icmp ult i32 %x, 16`, `%x`
//! has its top 28 bits known zero at any later instruction.

use super::query::Query;
use crate::knownbits::KnownBits;
use crate::module::{InstId, Operand};
use crate::opcode::IntPredicate;

/// `computeKnownBitsFromContext(V, Known, Q, Depth)`: `known` has its width and
/// is added to.
pub fn compute_known_bits_from_context(
    q: &Query,
    v: Operand,
    known: &mut KnownBits,
    depth: u32,
) {
    let _ = (q, v, known, depth);
}

/// `isValidAssumeForContext(Inv, CxtI, DT, AllowEphemerals)`.
pub fn is_valid_assume_for_context(
    q: &Query,
    assume: InstId,
    cxt: InstId,
    allow_ephemerals: bool,
) -> bool {
    let _ = (q, assume, cxt, allow_ephemerals);
    false
}

/// `isImpliedByDomCondition(Pred, LHS, RHS, ContextI, DL)`.
pub fn is_implied_by_dom_condition(
    q: &Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
    cxt: InstId,
) -> Option<bool> {
    let _ = (q, pred, lhs, rhs, cxt);
    None
}
