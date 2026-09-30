//! Exits their counts decide, folded: LLVM IndVarSimplify's
//! `optimizeLoopExits`. Of a loop's exits the latch follows, each counted
//! (`induction::exits`), in dominance order:
//!
//! - one whose count an earlier exit shares never leaves: the earlier one
//!   leaves on that trip first;
//! - one whose count is above the loop's most, the least of the others',
//!   as the constants or the guards on entry prove, never leaves;
//! - one whose count is zero leaves on the first trip.
//!
//! Its branch is given the condition that decides it; `decide` makes it a
//! jump. Nothing else changes, so no effect or value between the exits
//! matters.
//!
//! An exit no count decides, whose compare tests a counter stepping by one
//! against an invariant, is tested once instead, before the loop, where
//! the guards prove it holds on every trip up to the loop's most once it
//! holds on the first: LLVM's `optimizeLoopExitWithUnknownExitCount`. The
//! test is of the counter's start; failing it, the loop leaves on the first
//! trip, as it did.

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, AffineOperand, ExitCount, Linear};
use llrm_analysis::{cfg, guards, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::Outer;
use num_bigint::BigInt;

use crate::counting;

/// How an exit's branch is decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decided {
    Stays,
    Leaves,
}

/// A compare an exit branches on, tested once before its loop: `left
/// predicate right`, where the branch stays while it holds.
struct Hoisted {
    branch: InstId,
    exit: i64,
    predicate: IntPredicate,
    left: AffineOperand,
    right: Operand,
    before: InstId,
}

/// Every loop's exits their counts decide, folded, and those tested once
/// hoisted; whether any was.
pub fn folded(context: &mut Context, layout: &DataLayout, function: &mut Function, outer: &Outer) -> bool {
    let (decided, hoisted) = {
        let unit = memory::Unit::within(context, layout, function, outer);
        let facts = unit.registers();
        let loops = unit.shape().loops.clone();
        let mut decided = Vec::new();
        let mut hoisted = Vec::new();
        for loop_ in &loops {
            let exits = induction::exits(&unit, loop_, Some(&facts), false);
            decided.extend(_decided(&unit, loop_, &exits));
            hoisted.extend(_hoisted(&unit, loop_, &exits));
        }
        (decided, hoisted)
    };
    for one in &hoisted {
        let stays_on_true = !matches!(function.instruction(one.branch).operands[..], [_, Operand::Block(yes), _] if cfg::id(yes) == one.exit);
        let predicate = if stays_on_true { one.predicate } else { one.predicate.inverse() };
        let left = match &one.left {
            AffineOperand::Value(value, _) => Operand::Value(*value),
            AffineOperand::Const(known) => counting::constant(context, &known.n, known.width),
        };
        let bit = context.types.int(1);
        let test = function.create_instruction(Opcode::ICmp(predicate), bit, vec![left, one.right], Flags::default(), None);
        function.insert(test, Position::Before(one.before)).expect("a preheader's branch");
        let replaced = function.instruction(one.branch).operands[0];
        function.set_operand(one.branch, 0, Operand::Value(function.instruction(test).result.expect("a value")));
        if let Operand::Value(old) = replaced
            && function.users(old).is_empty()
            && let llrm_mir::module::ValueDef::Instruction(inst) = function.value(old).def
        {
            function.erase(inst).expect("an unused compare");
        }
    }
    for &(branch, exit, way) in &decided {
        let stays_on_true = !matches!(function.instruction(branch).operands[..], [_, Operand::Block(yes), _] if cfg::id(yes) == exit);
        let holds = (way == Decided::Stays) == stays_on_true;
        let condition = counting::constant(context, &BigInt::from(u8::from(holds)), 1);
        function.set_operand(branch, 0, condition);
    }
    !decided.is_empty() || !hoisted.is_empty()
}

/// The exits of `loop_` no count decides that one test before it may.
fn _hoisted(unit: &memory::Unit, loop_: &Loop, exits: &[ExitCount]) -> Vec<Hoisted> {
    let function = unit.function;
    let shape = unit.shape();
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return Vec::new() };
    let header = cfg::block(loop_.header);
    let outside = function.predecessors(header).into_iter().filter(|&one| !loop_.body.contains(&cfg::id(one))).collect::<Vec<_>>();
    let [preheader] = outside[..] else { return Vec::new() };
    let Some(before) = function.terminator(preheader) else { return Vec::new() };
    let counters = induction::basics(unit, loop_);
    let still = induction::invariant(function, &loop_.body);
    let most = induction::most_backedges(exits);
    let mut found = Vec::new();
    let mut earlier: Vec<Linear> = Vec::new();
    for exit in exits {
        if let Some(taken) = &exit.taken {
            earlier.extend(taken.iter().cloned());
            continue;
        }
        if !shape.dominance.dominates(exit.block, latch) {
            continue;
        }
        let [Operand::Value(condition), Operand::Block(yes), _] = function.instruction(exit.branch).operands[..] else { continue };
        let Some((_, compare)) = unit.defining(Operand::Value(condition)) else { continue };
        let (Opcode::ICmp(predicate), [one, other]) = (&compare.opcode, &compare.operands[..]) else { continue };
        if function.users(condition).len() != 1 || matches!(predicate, IntPredicate::Eq | IntPredicate::Ne) {
            continue;
        }
        let stays = cfg::id(yes) != exit.exit;
        let continuing = if stays { *predicate } else { predicate.inverse() };
        // The counter on the left, the invariant on the right.
        let (counter, right, predicate) = match (*one, *other) {
            (Operand::Value(value), right) if counters.contains_key(&value) => (value, right, continuing),
            (left, Operand::Value(value)) if counters.contains_key(&value) => (value, left, continuing.swapped()),
            _ => continue,
        };
        if matches!(right, Operand::Value(value) if !still.contains(value)) {
            continue;
        }
        let affine = &counters[&counter];
        let (Some(width), Some(right_term)) = (unit.int_bits(Operand::Value(counter)), induction::term(unit, right)) else { continue };
        let step = Linear::of(&affine.step, width);
        let Some(by) = step.known().filter(|by| by.magnitude() == &num_bigint::BigUint::from(1_u8)) else { continue };
        let start = Linear::of(&affine.start, width);
        let bound = Linear::of(&right_term, width);
        let signed = matches!(predicate, IntPredicate::Slt | IntPredicate::Sle | IntPredicate::Sgt | IntPredicate::Sge);
        let rising = by > BigInt::from(0);
        let no_wrap = match (signed, rising) {
            (true, true) => IntPredicate::Sle,
            (true, false) => IntPredicate::Sge,
            (false, true) => IntPredicate::Ule,
            (false, false) => IntPredicate::Uge,
        };
        let holds_up_to = |trips: &Linear| {
            let last = start.plus(&trips.times(&by));
            guards::holds(unit, latch, predicate, &last, &bound) && guards::holds(unit, exit.block, no_wrap, &start, &last)
        };
        // An earlier exit leaving on the loop's last trip spares the later ones it.
        let invariant = most.iter().filter(|one| one.width == width).any(|trips| holds_up_to(trips) || (earlier.contains(trips) && holds_up_to(&trips.minus(&Linear::constant(1, width)))));
        if invariant {
            found.push(Hoisted { branch: exit.branch, exit: exit.exit, predicate, left: affine.start.clone(), right, before });
        }
    }
    found
}

/// The branches of `loop_`'s exits their counts decide, and their exits.
fn _decided(unit: &memory::Unit, loop_: &Loop, exits: &[ExitCount]) -> Vec<(InstId, i64, Decided)> {
    let function = unit.function;
    // Only an exit of this loop and no inner one decides this loop's trips.
    let innermost = |at: i64| unit.shape().loops.iter().filter(|one| one.body.contains(&at)).all(|one| one.body.len() >= loop_.body.len());
    let mut decided = Vec::new();
    let mut earlier: Vec<Vec<Linear>> = Vec::new();
    for (index, exit) in exits.iter().enumerate() {
        let Some(taken) = &exit.taken else { continue };
        let condition = function.instruction(exit.branch).operands.first().copied();
        if !innermost(exit.block) || matches!(condition, Some(Operand::Constant(_))) {
            earlier.push(taken.clone());
            continue;
        }
        let others = exits.iter().enumerate().filter(|(at, _)| *at != index).filter_map(|(_, one)| one.taken.clone()).flatten().collect::<Vec<_>>();
        let way = if taken.iter().any(|one| one.known() == Some(BigInt::from(0))) {
            Some(Decided::Leaves)
        } else if earlier.iter().any(|one| one == taken) {
            Some(Decided::Stays)
        } else if others.iter().any(|most| taken.iter().all(|count| _below(unit, loop_.header, most, count))) {
            Some(Decided::Stays)
        } else {
            None
        };
        if let Some(way) = way {
            decided.push((exit.branch, exit.exit, way));
        }
        earlier.push(taken.clone());
    }
    decided
}

/// Whether `most` is below `count` as unsigned numbers on entry to `header`'s loop.
fn _below(unit: &memory::Unit, header: i64, most: &Linear, count: &Linear) -> bool {
    if most.width != count.width {
        return false;
    }
    if let (Some(one), Some(other)) = (most.known(), count.known()) {
        return guards::evaluated(IntPredicate::Ult, &one, &other, most.width);
    }
    guards::holds(unit, header, IntPredicate::Ult, most, count)
}

#[cfg(test)]
#[path = "exitfold_tests.rs"]
mod tests;
