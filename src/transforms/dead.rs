//! Removal of unused, side-effect-free portable IR instructions.
//!
//! This pass deliberately relies only on the IR's explicit effect summary.
//! It does not infer facts about calls, aliases, control flow, or traps: an
//! instruction is eligible only when that summary proves it is pure and every
//! value it defines has no remaining use.  Calls are retained because the
//! current summary does not prove that a call returns.

use std::collections::BTreeMap;

use crate::ir::{Callee, Function, Instruction, InstructionKind, Operand, Terminator, ValueId};

use super::{FunctionPass, PassFailure, PassOutcome, PreservedAnalyses};

/// Removes dead instructions whose effects are explicitly known to be pure.
///
/// Uses are recomputed after each removal round so an unused producer chain
/// disappears without relying on instruction order.  No CFG or phi edge is
/// changed; phi operands are counted as ordinary uses.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeadCodeElimination;

impl DeadCodeElimination {
    /// Creates a dead-instruction elimination pass.
    pub const fn new() -> Self {
        Self
    }
}

impl FunctionPass for DeadCodeElimination {
    fn name(&self) -> &'static str {
        "dead-code-elimination"
    }

    fn run(&mut self, function: &mut Function) -> Result<PassOutcome, PassFailure> {
        let mut changed = false;

        loop {
            let uses = count_uses(function);
            let mut removed = false;

            for block in &mut function.blocks {
                let instructions = std::mem::take(&mut block.instructions);
                let mut retained = Vec::with_capacity(instructions.len());

                for instruction in instructions {
                    if is_dead(&instruction, &uses) {
                        removed = true;
                    } else {
                        retained.push(instruction);
                    }
                }

                block.instructions = retained;
            }

            changed |= removed;
            if !removed {
                break;
            }
        }

        Ok(if changed {
            PassOutcome::changed(PreservedAnalyses::None)
        } else {
            PassOutcome::unchanged()
        })
    }
}

fn is_dead(instruction: &Instruction, uses: &BTreeMap<ValueId, usize>) -> bool {
    !matches!(&instruction.kind, InstructionKind::Call { .. })
        && instruction.effects().is_pure()
        && instruction
            .results
            .iter()
            .all(|result| !uses.contains_key(&result.id))
}

fn count_uses(function: &Function) -> BTreeMap<ValueId, usize> {
    let mut uses = BTreeMap::new();

    for block in &function.blocks {
        for instruction in &block.instructions {
            count_instruction_uses(instruction, &mut uses);
        }
        count_terminator_uses(&block.terminator, &mut uses);
    }

    uses
}

fn count_instruction_uses(instruction: &Instruction, uses: &mut BTreeMap<ValueId, usize>) {
    match &instruction.kind {
        InstructionKind::StackAlloc { .. } | InstructionKind::ParameterAddress { .. } => {}
        InstructionKind::Phi { incoming } => {
            for incoming in incoming {
                count_operand(&incoming.value, uses);
            }
        }
        InstructionKind::Unary { operand, .. } | InstructionKind::Cast { operand, .. } => {
            count_operand(operand, uses);
        }
        InstructionKind::Binary { left, right, .. }
        | InstructionKind::Compare { left, right, .. } => {
            count_operand(left, uses);
            count_operand(right, uses);
        }
        InstructionKind::Load { address, .. } => count_operand(address, uses),
        InstructionKind::Store { address, value, .. } => {
            count_operand(address, uses);
            count_operand(value, uses);
        }
        InstructionKind::ComposePointer { segment, offset } => {
            count_operand(segment, uses);
            count_operand(offset, uses);
        }
        InstructionKind::GetElementPointer { base, indices } => {
            count_operand(base, uses);
            for index in indices {
                count_operand(index, uses);
            }
        }
        InstructionKind::Select {
            condition,
            then_value,
            else_value,
        } => {
            count_operand(condition, uses);
            count_operand(then_value, uses);
            count_operand(else_value, uses);
        }
        InstructionKind::Call {
            callee, arguments, ..
        } => {
            if let Callee::Indirect(operand) = callee {
                count_operand(operand, uses);
            }
            for argument in arguments {
                count_operand(argument, uses);
            }
        }
        InstructionKind::Intrinsic { arguments, .. } => {
            for argument in arguments {
                count_operand(argument, uses);
            }
        }
    }
}

fn count_terminator_uses(terminator: &Terminator, uses: &mut BTreeMap<ValueId, usize>) {
    match terminator {
        Terminator::Jump(_) | Terminator::Unreachable => {}
        Terminator::Branch { condition, .. } => count_operand(condition, uses),
        Terminator::Switch { selector, .. } => count_operand(selector, uses),
        Terminator::Return(value) => {
            if let Some(value) = value {
                count_operand(value, uses);
            }
        }
    }
}

fn count_operand(operand: &Operand, uses: &mut BTreeMap<ValueId, usize>) {
    if let Operand::Value(value) = operand {
        *uses.entry(*value).or_insert(0) += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BinaryOp, Block, BlockId, CallingConvention, Constant, FunctionId, InstructionId, Linkage,
        Signature, TypeId, TypedConstant, Value,
    };

    const I8: TypeId = TypeId::new(0);

    fn integer(value: i128) -> Operand {
        Operand::Constant(TypedConstant {
            type_id: I8,
            value: Constant::Integer(value),
        })
    }

    fn value(id: u32) -> Value {
        Value {
            id: ValueId::new(id),
            type_id: I8,
        }
    }

    fn function(instructions: Vec<Instruction>, terminator: Terminator) -> Function {
        Function {
            id: FunctionId::new(0),
            name: "main".into(),
            signature: Signature {
                result: I8,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: CallingConvention::C,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions,
                terminator,
            }],
        }
    }

    fn add(id: u32, left: Operand, right: Operand) -> Instruction {
        Instruction {
            id: InstructionId::new(id),
            results: vec![value(id)],
            kind: InstructionKind::Binary {
                op: BinaryOp::Add,
                left,
                right,
            },
        }
    }

    #[test]
    fn removes_a_dead_pure_producer_chain() {
        let first = add(0, integer(1), integer(2));
        let second = add(1, Operand::Value(ValueId::new(0)), integer(3));
        let mut function = function(vec![first, second], Terminator::Return(Some(integer(0))));

        let outcome = DeadCodeElimination::new().run(&mut function).unwrap();

        assert!(outcome.changed_ir());
        assert!(function.blocks[0].instructions.is_empty());
    }

    #[test]
    fn retains_a_pure_producer_with_a_terminator_use() {
        let result = value(0);
        let mut function = function(
            vec![add(0, integer(1), integer(2))],
            Terminator::Return(Some(Operand::Value(result.id))),
        );

        let outcome = DeadCodeElimination::new().run(&mut function).unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(function.blocks[0].instructions.len(), 1);
    }

    #[test]
    fn retains_an_effectful_store_without_results() {
        let mut function = function(
            vec![Instruction {
                id: InstructionId::new(0),
                results: Vec::new(),
                kind: InstructionKind::Store {
                    address: integer(0),
                    value: integer(7),
                    alignment: 1,
                    volatile: false,
                },
            }],
            Terminator::Return(Some(integer(0))),
        );

        let outcome = DeadCodeElimination::new().run(&mut function).unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(function.blocks[0].instructions.len(), 1);
    }
}
