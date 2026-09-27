//! Counter comparisons settled by the counter's range, as LLVM's
//! IndVarSimplify eliminates them: where a loop stays only while a counter
//! from a constant start stays below a constant bound, a comparison of
//! that counter with a constant, in the part of the loop the test guards,
//! holds everywhere or nowhere.

use crate::context::{signed, Constant, ConstantKind};
use crate::module::{Operand, ValueDef};
use crate::opcode::{IntPredicate, Opcode};
use crate::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, ScalarEvolution, Unit};
use crate::scalarevolution::counted;

pub struct IndVars;

impl FunctionPass for IndVars {
    fn name(&self) -> &'static str {
        "indvars"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let loops = analyses.get::<Loops>(unit.context, unit.layout, unit.function);
        let evolution = analyses.get::<ScalarEvolution>(unit.context, unit.layout, unit.function);
        let mut changed = false;
        for one in &loops.loops {
            let Some(found) = counted(unit.context, unit.function, &tree, &evolution, one) else { continue };
            let Some(range) = range(unit, &evolution, &found) else { continue };
            let guarded: Vec<_> = unit.function.walk().filter(|&(block, _)| one.blocks.contains(&block) && tree.dominates(found.inside, block)).map(|(_, inst)| inst).collect();
            for inst in guarded {
                let instruction = unit.function.instruction(inst);
                let Opcode::ICmp(predicate) = instruction.opcode else { continue };
                let (predicate, other) = match instruction.operands[..] {
                    [Operand::Value(value), other] if value == found.counter => (predicate, other),
                    [other, Operand::Value(value)] if value == found.counter => (predicate.swapped(), other),
                    _ => continue,
                };
                let Some(bits) = constant(unit, other) else { continue };
                let width = unit.context.types.int_bits(unit.function.value(found.counter).ty).expect("an integer counter");
                let Some(truth) = settled(predicate, range, bits, width) else { continue };
                let ty = instruction.ty;
                let known = unit.context.constant(Constant { ty, kind: ConstantKind::Int(u128::from(truth)) });
                let result = instruction.result.expect("a comparison's value");
                unit.function.replace_all_uses_with(result, Operand::Constant(known));
                unit.function.erase(inst).expect("its uses were replaced");
                changed = true;
            }
        }
        // Blocks and edges are as they were.
        if changed { PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>() } else { PreservedAnalyses::all() }
    }
}

fn constant(unit: &Unit, operand: Operand) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match unit.context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        _ => None,
    }
}

/// The counter's values where the test guards, lowest and highest, as
/// signed integers: from its constant start up to below the bound.
fn range(unit: &Unit, evolution: &crate::scalarevolution::Evolution, found: &crate::scalarevolution::Counted) -> Option<(i128, i128)> {
    let ValueDef::Instruction(_) = unit.function.value(found.counter).def else { return None };
    let width = unit.context.types.int_bits(unit.function.value(found.counter).ty)?;
    let recurrence = evolution.of(found.counter)?;
    if unit.function.instruction(match unit.function.value(found.counter).def {
        ValueDef::Instruction(inst) => inst,
        ValueDef::Argument(_) => return None,
    }).opcode != Opcode::Phi || !recurrence.start.terms.is_empty() {
        return None;
    }
    let start = signed(recurrence.start.constant, width);
    let bound = signed(constant(unit, found.bound)?, width);
    // Counting up from a non-negative start, signed and unsigned agree.
    match (found.step, found.stays) {
        (1, IntPredicate::Slt) if start >= 0 && start < bound => Some((start, bound - 1)),
        (1, IntPredicate::Ult) if start >= 0 && bound >= 0 && start < bound => Some((start, bound - 1)),
        _ => None,
    }
}

/// Whether `predicate` against `bits` holds for every value in `range`
/// or for none.
fn settled(predicate: IntPredicate, (low, high): (i128, i128), bits: u128, width: u32) -> Option<bool> {
    let value = signed(bits, width);
    let unsigned = |value: i128| (value >= 0).then_some(value);
    let (holds_low, holds_high) = match predicate {
        IntPredicate::Slt => (low < value, high < value),
        IntPredicate::Sle => (low <= value, high <= value),
        IntPredicate::Sgt => (low > value, high > value),
        IntPredicate::Sge => (low >= value, high >= value),
        IntPredicate::Ult | IntPredicate::Ule | IntPredicate::Ugt | IntPredicate::Uge => {
            let value = unsigned(value)?;
            match predicate {
                IntPredicate::Ult => (low < value, high < value),
                IntPredicate::Ule => (low <= value, high <= value),
                IntPredicate::Ugt => (low > value, high > value),
                _ => (low >= value, high >= value),
            }
        }
        IntPredicate::Eq | IntPredicate::Ne => {
            let outside = value < low || value > high;
            return outside.then_some(predicate == IntPredicate::Ne);
        }
    };
    (holds_low == holds_high).then_some(holds_low)
}
