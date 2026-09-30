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

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, ExitCount, Linear};
use llrm_analysis::{cfg, guards, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::IntPredicate;
use llrm_mir::passes::Outer;
use num_bigint::BigInt;

use crate::counting;

/// How an exit's branch is decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decided {
    Stays,
    Leaves,
}

/// Every loop's exits their counts decide, folded; whether any was.
pub fn folded(context: &mut Context, layout: &DataLayout, function: &mut Function, outer: &Outer) -> bool {
    let decided = {
        let unit = memory::Unit::within(context, layout, function, outer);
        let facts = unit.registers();
        let loops = unit.shape().loops.clone();
        loops.iter().flat_map(|loop_| _decided(&unit, loop_, &induction::exits(&unit, loop_, Some(&facts), false))).collect::<Vec<_>>()
    };
    for &(branch, exit, way) in &decided {
        let stays_on_true = !matches!(function.instruction(branch).operands[..], [_, Operand::Block(yes), _] if cfg::id(yes) == exit);
        let holds = (way == Decided::Stays) == stays_on_true;
        let condition = counting::constant(context, &BigInt::from(u8::from(holds)), 1);
        function.set_operand(branch, 0, condition);
    }
    !decided.is_empty()
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
