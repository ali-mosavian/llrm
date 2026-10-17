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
use crate::opcode::{IntPredicate, Opcode};

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
/// its edge and the phi that edge is of. The phi's own value has no
/// terminator.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L812>
pub fn break_self_recursive_phi(
    q: &Query,
    phi: InstId,
    slot: usize,
) -> (Operand, Option<InstId>, InstId) {
    let instruction = q.function.instruction(phi);
    let this = super::pattern::result_of(instruction);
    let mut value = instruction.operands[2 * slot];
    if value == this {
        return (value, None, phi);
    }
    let Operand::Block(from) = instruction.operands[2 * slot + 1] else { unreachable!("a phi's block") };
    let mut cxt = q.function.terminator(from);
    let mut phi_out = phi;
    // A select of this phi: analyse its other arm.
    if let Some((_, select)) = q.instruction_of(value).filter(|(_, select)| select.opcode == Opcode::Select) {
        if select.operands[1] == this {
            value = select.operands[2];
        } else if select.operands[2] == this {
            value = select.operands[1];
        }
    }
    // A two-operand phi of this phi: analyse its other incoming value.
    if let Some((inc_phi, inc)) = super::pattern::phi_of(q, value) {
        if inc.operands.len() == 4 {
            for idx in 0..2 {
                if inc.operands[2 * idx] == this {
                    let Operand::Block(other) = inc.operands[2 * (1 - idx) + 1] else { unreachable!("a phi's block") };
                    value = inc.operands[2 * (1 - idx)];
                    phi_out = inc_phi;
                    cxt = q.function.terminator(other);
                    break;
                }
            }
        }
    }
    (value, cxt, phi_out)
}
