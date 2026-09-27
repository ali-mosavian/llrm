//! A value affine in a loop counter, written as the counter scaled plus one
//! invariant base: `counter * by + base`, as LLVM's LoopStrengthReduce
//! states a formula (a scaled register plus base registers).
//!
//! `((x - k) + 640) * 2` becomes `x * 2 + base`, with `base` built from the
//! invariant terms. Every such value of one counter and scale then shares
//! `x * 2`, and hoisting moves each `base` out of the loop. `induction`
//! owns the affine form; this only spells it.
//!
//! Adapted from llrm-core's `optimize/affine.rs`. A value is read through
//! no copies here, so an already spelled `scaled + base` is one whose
//! operand is its base. What was replaced is left for `dead`.
//!
//! Dropped: `transform::motion_blocked`, the old hoist's refusal of a loop
//! holding a call, which clobbered the machine's registers. A base is
//! arithmetic on values the loop does not define, which the ported hoist
//! moves across any call. The old module had no tests.

use std::collections::BTreeMap;

use llrm_analysis::induction::{self, AffineOperand, Derived};
use llrm_analysis::manager::Held;
use llrm_analysis::{cfg, memory};
use llrm_analysis::graph::loops::Loop;
use llrm_mir::module::{InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use num_bigint::BigInt;

use crate::dead;
use crate::strength::{_emitted, _operand, _starts};

pub struct Affine;

impl FunctionPass for Affine {
    fn name(&self) -> &'static str {
        "affine"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        if canonical(unit, analyses) {
            dead::dead(unit.context, analyses.outer().callees(), unit.function);
            // Blocks and edges are as they were.
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Each loop-affine value built from its scaled counter and one invariant
/// base; whether any was.
pub fn canonical(unit: &mut Unit, analyses: &mut Analyses) -> bool {
    let rewrites = {
        let held = Held::of(unit.context, unit.layout, unit.function, analyses, false);
        let view = held.unit(unit.context, unit.layout, unit.function, analyses.outer());
        let found = induction::of(&view);
        // A value is affine in every loop around it; its innermost loop is
        // the one whose trips it varies with.
        let mut innermost = BTreeMap::<InstId, (&Loop, &Derived)>::new();
        for (loop_, _, derived) in &found {
            for one in derived {
                if !innermost.get(&one.op).is_some_and(|(other, _)| other.body.len() <= loop_.body.len()) {
                    innermost.insert(one.op, (loop_, one));
                }
            }
        }
        innermost
            .into_values()
            .filter_map(|(loop_, one)| Some((_rewritable(&view, loop_, one)?, one.clone())))
            .collect::<Vec<_>>()
    };
    for ((counter, answer), one) in &rewrites {
        let ty = unit.function.value(*answer).ty;
        let scaled = match &one.by {
            AffineOperand::Const(by) if by.n == BigInt::from(1) => Operand::Value(*counter),
            by => {
                let by = _operand(unit, by);
                _emitted(unit, Opcode::Binary(BinaryOp::Mul), ty, vec![Operand::Value(*counter), by], one.op)
            }
        };
        let base = _starts(unit, one, false, one.op);
        let sum = _emitted(unit, Opcode::Binary(BinaryOp::Add), ty, vec![scaled, base], one.op);
        unit.function.replace_all_uses_with(*answer, sum);
    }
    !rewrites.is_empty()
}

/// The counter and result `one` is rewritten through, when its affine form
/// has an invariant value term and is not already `counter * by + base`.
fn _rewritable(view: &memory::Unit, loop_: &Loop, one: &Derived) -> Option<(ValueId, ValueId)> {
    let function = view.function;
    let answer = function.instruction(one.op).result?;
    if one.pointer.is_some()
        || view.int_bits(Operand::Value(answer)) != Some(one.of.start.width())
        || !one.offsets.iter().any(|(offset, _)| matches!(offset, AffineOperand::Value(..)))
    {
        return None;
    }
    let ValueDef::Instruction(phi) = function.value(one.of.value).def else { return None };
    if function.instruction(phi).opcode != Opcode::Phi || function.parent(phi) != Some(cfg::block(loop_.header)) {
        return None;
    }
    // Already `scaled + base`: one value offset, added.
    let op = function.instruction(one.op);
    let spelled = op.opcode == Opcode::Binary(BinaryOp::Add)
        && matches!(&one.offsets[..], [(AffineOperand::Value(base, _), coefficient)] if *coefficient == BigInt::from(1) && op.operands.contains(&Operand::Value(*base)));
    (!spelled).then_some((one.of.value, answer))
}

#[cfg(test)]
#[path = "affine_tests.rs"]
mod tests;
