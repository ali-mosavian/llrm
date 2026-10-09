//! What the branches that dominate a block prove on entry to it: the
//! compare of each branch whose edge toward the block alone reaches it,
//! and the `and`'s or `or`'s parts it joins. LLVM's
//! `isBasicBlockEntryGuardedByCond` as far as direct implication goes: a
//! guard proves a test of the same two sides that it is at least as
//! strong as, either way round.

use std::collections::BTreeMap;

use llrm_mir::module::{Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, IntPredicate, Opcode};
use num_bigint::BigInt;

use crate::cfg;
use crate::difference;
use crate::ranges::declared;
use crate::induction::{Scev, term};
use crate::memory::Unit;

/// `left predicate right`, proven on some edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Guard {
    pub predicate: IntPredicate,
    pub left: Scev,
    pub right: Scev,
}

#[cfg(test)]
thread_local! {
    static FOUND: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has found the guards of a block, for a test that a block asked of again is not found again.
#[cfg(test)]
pub(crate) fn found() -> usize {
    FOUND.with(std::cell::Cell::get)
}

/// The compares proven on entry to block `at`.
pub fn guards(unit: &Unit, at: i64) -> Vec<Guard> {
    #[cfg(test)]
    FOUND.with(|found| found.set(found.get() + 1));
    let function = unit.function;
    let shape = unit.shape();
    let assumed = unit.assumptions();
    let mut found = Vec::new();
    let mut reached = at;
    while let Some(above) = shape.dominance.immediate(reached) {
        // What the block assumes holds below it, wherever its terminator goes.
        for condition in assumed.here(above) {
            if let Operand::Value(condition) = *condition {
                _proven(unit, condition, true, &mut found);
            }
        }
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
            found.push(Guard { predicate, left: Scev::of(&left, width), right: Scev::of(&right, width) });
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
pub fn holds(unit: &Unit, at: i64, predicate: IntPredicate, left: &Scev, right: &Scev) -> bool {
    if let (Some(one), Some(other)) = (left.known(), right.known()) {
        return evaluated(predicate, &one, &other, left.width);
    }
    holds_in(unit, at, &guards(unit, at), predicate, left, right)
}

/// `holds`, given the guards on entry to `at` (`guards`), which a caller asking of one block again and again finds once: they were
/// found four times for each branch `decide` asked of (half of its time on QCport's sc).
fn holds_in(unit: &Unit, at: i64, facts: &[Guard], predicate: IntPredicate, left: &Scev, right: &Scev) -> bool {
    if let (Some(one), Some(other)) = (left.known(), right.known()) {
        return evaluated(predicate, &one, &other, left.width);
    }
    if facts.iter().any(|guard| implies(guard, predicate, left, right)) {
        return true;
    }
    // An unsigned compare of zero extensions is the compare of what they extend: the guard may be in the narrow type.
    if let Some((left, right)) = narrowed(unit, predicate, left, right)
        && (facts.iter().any(|guard| implies(guard, predicate, &left, &right)) || holds_in(unit, at, facts, predicate, &left, &right))
    {
        return true;
    }
    through_phi(unit, at, predicate, left, right)
}

/// `phi predicate right` below the phi's block where each edge into the block proves it of the value it brings: no branch dominates the
/// block then, as in a loop entered from a guard and from its own latch test (LLVM's `isImpliedCondition` over a header phi's incoming values).
fn through_phi(unit: &Unit, at: i64, predicate: IntPredicate, left: &Scev, right: &Scev) -> bool {
    let function = unit.function;
    let shape = unit.shape();
    let phi_of = |one: &Scev| -> Option<llrm_mir::module::InstId> {
        let [(product, factor)] = one.terms.iter().collect::<Vec<_>>()[..] else { return None };
        if one.constant != BigInt::from(0) || *factor != BigInt::from(1) {
            return None;
        }
        let ValueDef::Instruction(inst) = function.value(product.single()?).def else { return None };
        (function.instruction(inst).opcode == Opcode::Phi).then_some(inst)
    };
    let (phi, from_left) = match (phi_of(left), phi_of(right)) {
        (Some(phi), None) => (phi, true),
        (None, Some(phi)) => (phi, false),
        _ => return false,
    };
    let Some(block) = function.parent(phi) else { return false };
    let other = if from_left { right } else { left };
    // The other side stays what it is on every trip: a constant, or a value defined above the phi's block.
    let fixed = other.known().is_some()
        || other.unknowns().all(|value| match function.value(value).def {
            ValueDef::Instruction(inst) => function.parent(inst).is_some_and(|home| home != block && shape.dominance.dominates(cfg::id(home), cfg::id(block))),
            _ => true,
        });
    if !fixed || !shape.dominance.dominates(cfg::id(block), at) {
        return false;
    }
    let operands = function.instruction(phi).operands.clone();
    if operands.len() < 2 {
        return false;
    }
    let width = left.width;
    operands.chunks(2).all(|arm| {
        let Operand::Block(from) = &arm[1] else { return false };
        let Some(brought) = term(unit, arm[0]) else { return false };
        let brought = Scev::of(&brought, width);
        let (l, r) = if from_left { (&brought, right) } else { (left, &brought) };
        if let (Some(one), Some(two)) = (l.known(), r.known()) {
            return evaluated(predicate, &one, &two, width);
        }
        // What holds at the end of the incoming block, and what its branch says of the edge into this one.
        let mut proven = guards(unit, cfg::id(*from));
        if let Some(branch) = function.terminator(*from)
            && let [Operand::Value(condition), Operand::Block(yes), Operand::Block(no)] = function.instruction(branch).operands[..]
            && yes != no
            && (yes == block || no == block)
        {
            _proven(unit, condition, yes == block, &mut proven);
        }
        proven.iter().any(|guard| implies(guard, predicate, l, r))
    })
}

/// The two sides read before the zero extension that made one of them, where each is that or a constant it holds: `zext a` against `64`
/// is `a` against `64` one width down. Unsigned and equality compares only, which a zero extension keeps.
fn narrowed(unit: &Unit, predicate: IntPredicate, left: &Scev, right: &Scev) -> Option<(Scev, Scev)> {
    use IntPredicate::*;
    if !matches!(predicate, Ult | Ule | Ugt | Uge | Eq | Ne) {
        return None;
    }
    let source = |one: &Scev| -> Option<(ValueId, u32)> {
        let [(product, factor)] = one.terms.iter().collect::<Vec<_>>()[..] else { return None };
        if one.constant != BigInt::from(0) || *factor != BigInt::from(1) {
            return None;
        }
        let value = product.single()?;
        let ValueDef::Instruction(inst) = unit.function.value(value).def else { return None };
        let op = unit.function.instruction(inst);
        let (Opcode::Cast(llrm_mir::opcode::CastOp::ZExt), [Operand::Value(from)]) = (&op.opcode, &op.operands[..]) else { return None };
        Some((*from, unit.int_bits(Operand::Value(*from))?))
    };
    let narrow = |one: &Scev, bits: u32| -> Option<Scev> {
        if let Some((from, width)) = source(one) {
            return (width == bits).then(|| Scev::of(&crate::induction::AffineOperand::Value(from, width), width));
        }
        let value = one.known().filter(|_| one.terms.is_empty())?;
        let unsigned = if value < BigInt::from(0) { value + (BigInt::from(1) << one.width) } else { value };
        (unsigned < (BigInt::from(1) << bits)).then(|| Scev::constant(unsigned, bits))
    };
    let bits = source(left).or_else(|| source(right))?.1;
    Some((narrow(left, bits)?, narrow(right, bits)?))
}

/// `holds`, given also `assumed` and what the program states of the
/// values the guards test: their ranges, by difference bounds.
pub fn holds_given(unit: &Unit, at: i64, assumed: &[Guard], predicate: IntPredicate, left: &Scev, right: &Scev) -> bool {
    Given::at(unit, at, assumed).holds(unit, predicate, left, right)
}

/// What is given on entry to a block: the guards there, and with the `assumed` ones and the loops' proofs of their counters'
/// followers the facts a range proof reads. Found once, for as many questions of the block as are asked.
pub struct Given<'a> {
    at: i64,
    guards: Vec<Guard>,
    assumed: &'a [Guard],
    facts: std::cell::OnceCell<Vec<Guard>>,
}

impl<'a> Given<'a> {
    pub fn at(unit: &Unit, at: i64, assumed: &'a [Guard]) -> Self {
        Self { at, guards: guards(unit, at), assumed, facts: std::cell::OnceCell::new() }
    }

    /// `holds_given`.
    pub fn holds(&self, unit: &Unit, predicate: IntPredicate, left: &Scev, right: &Scev) -> bool {
        if holds_in(unit, self.at, &self.guards, predicate, left, right) {
            return true;
        }
        let facts = self.facts.get_or_init(|| {
            let mut facts = self.guards.clone();
            facts.extend(self.assumed.iter().cloned());
            // What the loops holding the block prove of the phis that follow their counters.
            let shape = unit.shape();
            for loop_ in shape.loops.iter().filter(|one| one.body.contains(&self.at)) {
                for (follower, counter, start) in crate::induction::followers(unit, loop_) {
                    let Some(width) = unit.int_bits(Operand::Value(follower)) else { continue };
                    let (follower, counter) = (Scev::unknown(follower, width), Scev::unknown(counter, width));
                    facts.push(Guard { predicate: IntPredicate::Sle, left: Scev::of(&start, width), right: follower.clone() });
                    facts.push(Guard { predicate: IntPredicate::Sle, left: follower, right: counter });
                }
            }
            facts
        });
        let mut ranges = BTreeMap::new();
        for side in facts.iter().flat_map(|one| [&one.left, &one.right]).chain([left, right]) {
            for monomial in side.terms.keys() {
                if let Some(value) = monomial.single()
                    && let Some(interval) = declared(unit, value).filter(|one| one.width == left.width)
                {
                    ranges.insert(monomial.clone(), (interval.low, interval.high));
                }
            }
        }
        difference::proves(left.width, facts, &ranges, predicate, left, right)
    }
}

/// Whether `guard` proves `left predicate right`.
pub fn implies(guard: &Guard, predicate: IntPredicate, left: &Scev, right: &Scev) -> bool {
    // `a != 0` proves `0 <u a`: the guard of a loop that counts up from zero to `a`, as LLVM's isLoopEntryGuardedByCond finds it.
    if guard.predicate == IntPredicate::Ne {
        let zero = |one: &Scev| one.known().is_some_and(|value| value == BigInt::from(0));
        let nonzero = if zero(&guard.right) { Some(&guard.left) } else if zero(&guard.left) { Some(&guard.right) } else { None };
        if let Some(value) = nonzero {
            let below = (predicate == IntPredicate::Ult && zero(left) && right == value) || (predicate == IntPredicate::Ugt && zero(right) && left == value);
            if below {
                return true;
            }
        }
    }
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
