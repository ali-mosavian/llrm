//! Canonical forms, as LLVM's InstCombine puts them: compares with the
//! constant on the right, no neutral terms.
//!
//! Adapted from llrm-core's `optimize/canonical.rs`, the port of
//! `qbopt/optimize/canonical.py`. Folding makes `icmp C, v` whenever a
//! compare's left operand becomes known, and no machine compares an
//! immediate against a register in that order. The compare is swapped, its
//! predicate mirrored.
//!
//! What had meaning only in the old MIR is gone: a compare there was a
//! subtraction whose flags each reader tested, so one read by other than a
//! test got its constant copied into a value instead; here the predicate is
//! the compare's own, and every compare can be swapped. A compare of two
//! constants is left to folding. A neutral term's raised source bytes, and
//! its flags read by a carry, have no counterpart either.

use llrm_mir::context::{Context, ConstantKind, mask};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, Loops, PreservedAnalyses, Unit, Dominators};

/// `compares` then `identities`, as the old fold applied them.
pub struct Canonical;

impl FunctionPass for Canonical {
    fn name(&self) -> &'static str {
        "canonical"
    }

    fn run(&mut self, unit: &mut Unit, _: &mut Analyses) -> PreservedAnalyses {
        let swapped = compares(unit.context, unit.function);
        if identities(unit.context, unit.function) | swapped {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Whether any compare was swapped.
pub fn compares(context: &Context, function: &mut Function) -> bool {
    let swapped = function.walk().map(|(_, inst)| inst).filter(|&inst| _constant_left(context, function, inst)).collect::<Vec<_>>();
    for &inst in &swapped {
        let instruction = function.instruction(inst);
        let Opcode::ICmp(predicate) = instruction.opcode else { unreachable!("a compare") };
        let operands = vec![instruction.operands[1], instruction.operands[0]];
        replaced(function, inst, Opcode::ICmp(predicate.swapped()), operands);
    }
    !swapped.is_empty()
}

fn _constant_left(context: &Context, function: &Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    let constant = |operand| matches!(operand, Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(_)));
    matches!(instruction.opcode, Opcode::ICmp(_)) && constant(instruction.operands[0]) && !matches!(instruction.operands[1], Operand::Constant(_))
}

/// `inst` rebuilt with `opcode` and `operands`, where it was.
fn replaced(function: &mut Function, inst: InstId, opcode: Opcode, operands: Vec<Operand>) {
    let instruction = function.instruction(inst);
    let new = function.create_instruction(opcode, instruction.ty, operands, instruction.flags, None);
    function.insert(new, Position::Before(inst)).expect("a placed instruction");
    let (old, value) = (function.instruction(inst).result.expect("a value"), function.instruction(new).result.expect("a value"));
    function.replace_all_uses_with(old, Operand::Value(value));
    function.erase(inst).expect("its uses were replaced");
}

/// `x + 0`, `x - 0` and `x * 1` are `x`, and a test `x <=u 0` is `x == 0`.
/// Whether anything changed.
///
/// A rewrite states what it computes from a proof in full -- rotation's
/// trip count is `bound - start + inclusive` for any start -- and the
/// neutral terms go here, so no rewrite folds its own.
pub fn identities(context: &Context, function: &mut Function) -> bool {
    let mut changed = false;
    for inst in function.walk().map(|(_, inst)| inst).collect::<Vec<_>>() {
        if let Some(kept) = _neutral(context, function, inst) {
            let result = function.instruction(inst).result.expect("a value");
            function.replace_all_uses_with(result, kept);
            function.erase(inst).expect("its uses were replaced");
            changed = true;
        } else if _zero_test(context, function, inst) {
            let instruction = function.instruction(inst);
            // Nothing is below zero.
            let predicate = match instruction.opcode {
                Opcode::ICmp(IntPredicate::Ule) => IntPredicate::Eq,
                Opcode::ICmp(IntPredicate::Ugt) => IntPredicate::Ne,
                _ => continue,
            };
            let operands = instruction.operands.clone();
            replaced(function, inst, Opcode::ICmp(predicate), operands);
            changed = true;
        }
    }
    changed
}

/// The operand a pure `x + 0`, `x - 0` or `x * 1` passes through unchanged.
fn _neutral(context: &Context, function: &Function, inst: InstId) -> Option<Operand> {
    let instruction = function.instruction(inst);
    let identity = match instruction.opcode {
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) => 0,
        Opcode::Binary(BinaryOp::Mul) => 1,
        _ => return None,
    };
    let width = context.types.int_bits(instruction.ty)?;
    let (left, right) = (instruction.operands[0], instruction.operands[1]);
    let pairs = [(left, right), (right, left)];
    let commutes = instruction.opcode != Opcode::Binary(BinaryOp::Sub);
    for (kept, other) in &pairs[..1 + usize::from(commutes)] {
        if _integer(context, *other).is_some_and(|bits| bits & mask(width) == identity) {
            return Some(*kept);
        }
    }
    None
}

/// An `icmp` against zero on the right.
fn _zero_test(context: &Context, function: &Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    matches!(instruction.opcode, Opcode::ICmp(_)) && _integer(context, instruction.operands[1]) == Some(0)
}

/// An integer constant's bits.
fn _integer(context: &Context, operand: Operand) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
#[path = "canonical_tests.rs"]
mod tests;
