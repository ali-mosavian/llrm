//! Shared operand rewriting for portable IR transformations.

use std::collections::BTreeMap;

use crate::ir::{Callee, Function, Instruction, InstructionKind, Operand, Terminator, ValueId};

/// Replaces every use of a mapped SSA value with its replacement operand.
///
/// Definitions are left untouched. Callers own legality and type checks; this
/// helper only provides one exhaustive walk over all operand-bearing IR forms.
pub(crate) fn replace_value_uses(
    function: &mut Function,
    replacements: &BTreeMap<ValueId, Operand>,
) -> bool {
    let mut changed = false;
    for block in &mut function.blocks {
        for instruction in &mut block.instructions {
            changed |= rewrite_instruction(instruction, replacements);
        }
        changed |= rewrite_terminator(&mut block.terminator, replacements);
    }
    changed
}

fn rewrite_instruction(
    instruction: &mut Instruction,
    replacements: &BTreeMap<ValueId, Operand>,
) -> bool {
    let mut changed = false;
    match &mut instruction.kind {
        InstructionKind::StackAlloc { .. } | InstructionKind::ParameterAddress { .. } => {}
        InstructionKind::Phi { incoming } => {
            for incoming in incoming {
                changed |= rewrite_operand(&mut incoming.value, replacements);
            }
        }
        InstructionKind::Unary { operand, .. } | InstructionKind::Cast { operand, .. } => {
            changed |= rewrite_operand(operand, replacements);
        }
        InstructionKind::Binary { left, right, .. }
        | InstructionKind::Compare { left, right, .. } => {
            changed |= rewrite_operand(left, replacements);
            changed |= rewrite_operand(right, replacements);
        }
        InstructionKind::Load { address, .. } => {
            changed |= rewrite_operand(address, replacements);
        }
        InstructionKind::Store { address, value, .. } => {
            changed |= rewrite_operand(address, replacements);
            changed |= rewrite_operand(value, replacements);
        }
        InstructionKind::ComposePointer { segment, offset } => {
            changed |= rewrite_operand(segment, replacements);
            changed |= rewrite_operand(offset, replacements);
        }
        InstructionKind::GetElementPointer { base, indices } => {
            changed |= rewrite_operand(base, replacements);
            for index in indices {
                changed |= rewrite_operand(index, replacements);
            }
        }
        InstructionKind::Select {
            condition,
            then_value,
            else_value,
        } => {
            changed |= rewrite_operand(condition, replacements);
            changed |= rewrite_operand(then_value, replacements);
            changed |= rewrite_operand(else_value, replacements);
        }
        InstructionKind::Call {
            callee, arguments, ..
        } => {
            if let Callee::Indirect(operand) = callee {
                changed |= rewrite_operand(operand, replacements);
            }
            for argument in arguments {
                changed |= rewrite_operand(argument, replacements);
            }
        }
        InstructionKind::Intrinsic { arguments, .. } => {
            for argument in arguments {
                changed |= rewrite_operand(argument, replacements);
            }
        }
    }
    changed
}

fn rewrite_terminator(
    terminator: &mut Terminator,
    replacements: &BTreeMap<ValueId, Operand>,
) -> bool {
    match terminator {
        Terminator::Jump(_) | Terminator::Unreachable => false,
        Terminator::Branch { condition, .. } => rewrite_operand(condition, replacements),
        Terminator::Switch { selector, .. } => rewrite_operand(selector, replacements),
        Terminator::Return(value) => value
            .as_mut()
            .is_some_and(|operand| rewrite_operand(operand, replacements)),
    }
}

fn rewrite_operand(operand: &mut Operand, replacements: &BTreeMap<ValueId, Operand>) -> bool {
    let Operand::Value(value) = operand else {
        return false;
    };
    let Some(replacement) = replacements.get(value) else {
        return false;
    };
    operand.clone_from(replacement);
    true
}
