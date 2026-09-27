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
use llrm_support::hash::IndexMap;
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
        // Addresses one index already serves share it: each spelled off
        // its own base would hold one base more where it saves no add.
        let function = view.function;
        let mut indices = IndexMap::<_, usize>::default();
        for (loop_, one) in innermost.values().filter(|(_, one)| one.pointer.is_some()) {
            *indices.entry((loop_.header, function.instruction(one.op).operands[1..].to_vec())).or_default() += 1;
        }
        let alone = |loop_: &Loop, one: &Derived| one.pointer.is_none() || indices[&(loop_.header, function.instruction(one.op).operands[1..].to_vec())] == 1;
        innermost
            .into_values()
            .filter(|(loop_, one)| alone(loop_, one))
            .filter_map(|(loop_, one)| Some((_rewritable(&view, loop_, one)?, one.clone())))
            .collect::<Vec<_>>()
    };
    for ((counter, answer), one) in &rewrites {
        let ty = unit.function.value(*answer).ty;
        let int = unit.function.value(*counter).ty;
        let scaled = match &one.by {
            AffineOperand::Const(by) if by.n == BigInt::from(1) => Operand::Value(*counter),
            by => {
                let by = _operand(unit, by);
                _emitted(unit, Opcode::Binary(BinaryOp::Mul), int, vec![Operand::Value(*counter), by], one.op)
            }
        };
        let source = unit.context.types.int(8);
        let sum = match (one.pointer, _displacement(one)) {
            // A constant displacement goes last, where an addressing mode
            // takes it: an invariant base of its own would hold a register.
            (Some(pointer), Some(displacement)) => {
                let indexed = _emitted(unit, Opcode::GetElementPtr { source }, ty, vec![pointer, scaled], one.op);
                let displacement = _operand(unit, &displacement);
                _emitted(unit, Opcode::GetElementPtr { source }, ty, vec![indexed, displacement], one.op)
            }
            (Some(_), None) => {
                let base = _starts(unit, one, false, one.op);
                _emitted(unit, Opcode::GetElementPtr { source }, ty, vec![base, scaled], one.op)
            }
            (None, _) => {
                let base = _starts(unit, one, false, one.op);
                _emitted(unit, Opcode::Binary(BinaryOp::Add), ty, vec![scaled, base], one.op)
            }
        };
        unit.function.replace_all_uses_with(*answer, sum);
    }
    !rewrites.is_empty()
}

/// The counter and result `one` is rewritten through, when its affine form
/// has an invariant value term and is not already `counter * by + base`.
fn _rewritable(view: &memory::Unit, loop_: &Loop, one: &Derived) -> Option<(ValueId, ValueId)> {
    let function = view.function;
    let answer = function.instruction(one.op).result?;
    let width = match one.pointer {
        Some(_) => view.space(Operand::Value(answer)).map(|space| view.layout.pointer(space).index_bits),
        None => view.int_bits(Operand::Value(answer)),
    };
    if width != Some(one.of.start.width()) || !one.offsets.iter().any(|(offset, _)| matches!(offset, AffineOperand::Value(..))) && _displacement(one).is_none() {
        return None;
    }
    let ValueDef::Instruction(phi) = function.value(one.of.value).def else { return None };
    if function.instruction(phi).opcode != Opcode::Phi || function.parent(phi) != Some(cfg::block(loop_.header)) {
        return None;
    }
    // Already `scaled + base`: one value offset, added; an address, its
    // invariant base indexed by the scaled counter alone.
    let op = function.instruction(one.op);
    let scaled = |operand: Operand| {
        operand == Operand::Value(one.of.value)
            || view.defining(operand).is_some_and(|(_, made)| made.opcode == Opcode::Binary(BinaryOp::Mul) && made.operands.contains(&Operand::Value(one.of.value)))
    };
    let invariant = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => function.parent(def).is_some_and(|block| !loop_.body.contains(&cfg::id(block))),
            ValueDef::Argument(_) => true,
        },
        _ => true,
    };
    let indexed = |operand: Operand| matches!(view.defining(operand), Some((_, made)) if matches!(made.opcode, Opcode::GetElementPtr { .. }) && matches!(made.operands[..], [base, index] if invariant(base) && scaled(index)));
    let spelled = match one.pointer {
        Some(_) => {
            matches!(op.opcode, Opcode::GetElementPtr { .. })
                && match op.operands[..] {
                    [base, index] => invariant(base) && scaled(index) || matches!(index, Operand::Constant(_)) && indexed(base),
                    _ => false,
                }
        }
        None => {
            op.opcode == Opcode::Binary(BinaryOp::Add)
                && matches!(&one.offsets[..], [(AffineOperand::Value(base, _), coefficient)] if *coefficient == BigInt::from(1) && op.operands.contains(&Operand::Value(*base)))
        }
    };
    (!spelled).then_some((one.of.value, answer))
}

/// The bytes an address adds to its base and scaled counter, where every
/// other term is a nonzero constant.
fn _displacement(one: &Derived) -> Option<AffineOperand> {
    one.pointer?;
    let width = one.of.start.width();
    let mut sum = BigInt::from(0);
    for (term, coefficient) in &one.offsets {
        let AffineOperand::Const(known) = term else { return None };
        sum += &known.n * coefficient;
    }
    let modulus = BigInt::from(1) << width;
    let sum = (sum % &modulus + &modulus) % &modulus;
    (sum != BigInt::from(0)).then(|| AffineOperand::constant(sum, width))
}

#[cfg(test)]
#[path = "affine_tests.rs"]
mod tests;
