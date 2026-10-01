//! What the branches that dominate a block prove on entry to it: the
//! compare of each branch whose edge toward the block alone reaches it,
//! and the `and`'s or `or`'s parts it joins. LLVM's
//! `isBasicBlockEntryGuardedByCond` as far as direct implication goes: a
//! guard proves a test of the same two sides that it is at least as
//! strong as, either way round.

use llrm_mir::module::{Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, IntPredicate, Opcode};
use num_bigint::BigInt;

use crate::cfg;
use crate::induction::{Linear, term};
use crate::memory::Unit;

/// `left predicate right`, proven on some edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Guard {
    pub predicate: IntPredicate,
    pub left: Linear,
    pub right: Linear,
}

/// The compares proven on entry to block `at`.
pub fn guards(unit: &Unit, at: i64) -> Vec<Guard> {
    let function = unit.function;
    let shape = unit.shape();
    let mut found = Vec::new();
    let mut reached = at;
    while let Some(above) = shape.dominance.immediate(reached) {
        // The edge from `above` toward `at` that alone enters the block it leads to.
        if let Some(branch) = function.terminator(cfg::block(above))
            && let [Operand::Value(condition), Operand::Block(yes), Operand::Block(no)] = function.instruction(branch).operands[..]
            && yes != no
        {
            let alone = |to: llrm_mir::module::BlockId| shape.dominance.dominates(cfg::id(to), at) && function.predecessors(to).iter().all(|&from| from == cfg::block(above) || shape.dominance.dominates(cfg::id(to), cfg::id(from)));
            if alone(yes) {
                _proven(unit, condition, true, &mut found);
            } else if alone(no) {
                _proven(unit, condition, false, &mut found);
            }
        }
        reached = above;
    }
    found
}

/// The compares `condition` being `holds` proves: its own, or those of
/// the `and` it is true of, or the `or` it is false of.
fn _proven(unit: &Unit, condition: ValueId, holds: bool, found: &mut Vec<Guard>) {
    let function = unit.function;
    let ValueDef::Instruction(inst) = function.value(condition).def else { return };
    let op = function.instruction(inst);
    match (&op.opcode, &op.operands[..]) {
        (Opcode::ICmp(predicate), [left, right]) => {
            let (Some(left), Some(right), Some(width)) = (term(unit, *left), term(unit, *right), unit.int_bits(*left)) else { return };
            let predicate = if holds { *predicate } else { predicate.inverse() };
            found.push(Guard { predicate, left: Linear::of(&left, width), right: Linear::of(&right, width) });
        }
        (Opcode::Binary(kind @ (BinaryOp::And | BinaryOp::Or)), [Operand::Value(one), Operand::Value(other)]) if (*kind == BinaryOp::And) == holds => {
            _proven(unit, *one, holds, found);
            _proven(unit, *other, holds, found);
        }
        _ => {}
    }
}

/// Whether `left predicate right` holds on entry to block `at`: both
/// constants, or a guard there proves it.
pub fn holds(unit: &Unit, at: i64, predicate: IntPredicate, left: &Linear, right: &Linear) -> bool {
    if let (Some(one), Some(other)) = (left.known(), right.known()) {
        return evaluated(predicate, &one, &other, left.width);
    }
    guards(unit, at).iter().any(|guard| implies(guard, predicate, left, right))
}

/// Whether `guard` proves `left predicate right`.
pub fn implies(guard: &Guard, predicate: IntPredicate, left: &Linear, right: &Linear) -> bool {
    if guard.left == *left && guard.right == *right {
        return _stronger(guard.predicate, predicate);
    }
    guard.left == *right && guard.right == *left && _stronger(guard.predicate.swapped(), predicate)
}

/// Whether `strong` holding of two sides makes `weak` hold of them.
fn _stronger(strong: IntPredicate, weak: IntPredicate) -> bool {
    use IntPredicate::*;
    strong == weak
        || matches!(
            (strong, weak),
            (Eq, Ule | Uge | Sle | Sge) | (Ult, Ule | Ne) | (Ugt, Uge | Ne) | (Slt, Sle | Ne) | (Sgt, Sge | Ne)
        )
}

/// `one predicate other` of two signed numbers read at `width` bits.
pub fn evaluated(predicate: IntPredicate, one: &BigInt, other: &BigInt, width: u32) -> bool {
    use IntPredicate::*;
    let modulus = BigInt::from(1) << width;
    let unsigned = |n: &BigInt| ((n % &modulus) + &modulus) % &modulus;
    let (a, b) = (unsigned(one), unsigned(other));
    let half = BigInt::from(1) << (width - 1);
    let signed = |n: &BigInt| if *n >= half { n - &modulus } else { n.clone() };
    let (s, t) = (signed(&a), signed(&b));
    match predicate {
        Eq => a == b,
        Ne => a != b,
        Ult => a < b,
        Ule => a <= b,
        Ugt => a > b,
        Uge => a >= b,
        Slt => s < t,
        Sle => s <= t,
        Sgt => s > t,
        Sge => s >= t,
    }
}

#[cfg(test)]
#[path = "guards_tests.rs"]
mod tests;
