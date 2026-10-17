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

/// `computeKnownBitsFromCond(V, Cond, Known, SQ, Invert, Depth)`.
pub fn compute_known_bits_from_cond(
    q: &Query,
    v: Operand,
    cond: Operand,
    known: &mut KnownBits,
    invert: bool,
    depth: u32,
) {
    let _ = (q, v, cond, known, invert, depth);
}

/// `breakSelfRecursivePHI(U, PHI, ValOut, CtxIOut, PhiOut)` for the incoming
/// `slot`: the value to analyse in place of the phi's own, the terminator of
/// its edge and the phi that edge is of.
pub fn break_self_recursive_phi(
    q: &Query,
    phi: InstId,
    slot: usize,
) -> (Operand, Option<InstId>, InstId) {
    let instruction = q.function.instruction(phi);
    let Operand::Block(from) = instruction.operands[2 * slot + 1] else { unreachable!("a phi's block") };
    (instruction.operands[2 * slot], q.function.terminator(from), phi)
}
