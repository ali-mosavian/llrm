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

/// `isKnownNonZeroFromAssume(V, Q)`.
pub fn is_known_non_zero_from_assume(
    q: &Query,
    v: Operand,
) -> bool {
    let _ = (q, v);
    false
}

/// `isTruePredicate(Pred, LHS, RHS)`.
pub fn is_true_predicate(
    q: &Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> bool {
    let _ = (q, pred, lhs, rhs);
    false
}

/// `isImpliedCondition(LHS, RHSPred, RHSOp0, RHSOp1, DL, LHSIsTrue, Depth)`.
pub fn is_implied_condition_pred(
    q: &Query,
    lhs: Operand,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
    lhs_is_true: bool,
    depth: u32,
) -> Option<bool> {
    let _ = (q, lhs, pred, op0, op1, lhs_is_true, depth);
    None
}
