//! Same-basic-block common subexpression elimination.
//!
//! This intentionally small pass uses exact portable IR equality.  It does
//! not canonicalize operand order, reason about aliases, or move expressions
//! across control-flow boundaries.

use std::collections::{BTreeMap, BTreeSet};

use crate::ir::{Function, Instruction, InstructionKind, Operand, TypeId, ValueId};

use super::rewrite::replace_value_uses;
use super::{FunctionPass, PassFailure, PassOutcome, PreservedAnalyses};

/// Removes repeated pure, single-result expressions within one basic block.
#[derive(Clone, Copy, Debug, Default)]
pub struct CommonSubexpressionElimination;

impl CommonSubexpressionElimination {
    /// Creates a same-basic-block common subexpression elimination pass.
    pub const fn new() -> Self {
        Self
    }
}

impl FunctionPass for CommonSubexpressionElimination {
    fn name(&self) -> &'static str {
        "common-subexpression-elimination"
    }

    fn run(&mut self, function: &mut Function) -> Result<PassOutcome, PassFailure> {
        reject_duplicate_values(function)?;

        let mut replacements = BTreeMap::new();
        let mut changed = false;
        loop {
            let rewrote_uses = replace_value_uses(function, &replacements);
            let mut removed = false;

            for block in &mut function.blocks {
                let instructions = std::mem::take(&mut block.instructions);
                let mut retained = Vec::with_capacity(instructions.len());
                let mut available: Vec<AvailableExpression> = Vec::new();

                for instruction in instructions {
                    if let Some((result, expression)) = candidate(&instruction) {
                        if let Some(existing) = available.iter().find(|entry| {
                            entry.type_id == result.type_id && entry.kind == expression
                        }) {
                            replacements.insert(result.id, Operand::Value(existing.value));
                            removed = true;
                            continue;
                        }
                        available.push(AvailableExpression {
                            type_id: result.type_id,
                            kind: expression,
                            value: result.id,
                        });
                    }
                    retained.push(instruction);
                }
                block.instructions = retained;
            }

            changed |= rewrote_uses || removed;
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

#[derive(Clone, Debug)]
struct AvailableExpression {
    type_id: TypeId,
    kind: InstructionKind,
    value: ValueId,
}

fn candidate(instruction: &Instruction) -> Option<(&crate::ir::Value, InstructionKind)> {
    let [result] = instruction.results.as_slice() else {
        return None;
    };
    if !instruction.effects().is_pure()
        || matches!(
            &instruction.kind,
            InstructionKind::Call { .. } | InstructionKind::StackAlloc { .. }
        )
    {
        return None;
    }
    Some((result, instruction.kind.clone()))
}

fn reject_duplicate_values(function: &Function) -> Result<(), PassFailure> {
    let mut values = BTreeSet::new();
    for value in function.parameters.iter().chain(
        function
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .flat_map(|instruction| instruction.results.iter()),
    ) {
        if !values.insert(value.id) {
            return Err(PassFailure::new(format!(
                "function {} has duplicate value id {}",
                function.id, value.id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BinaryOp, Block, BlockId, Callee, CallingConvention, Constant, Effects, Function,
        FunctionId, InstructionId, Linkage, MemoryEffects, Signature, Terminator, Type, TypeKind,
        TypedConstant, Value,
    };

    const I8: TypeId = TypeId::new(0);
    const PTR: TypeId = TypeId::new(1);

    fn value(id: u32, type_id: TypeId) -> Value {
        Value {
            id: ValueId::new(id),
            type_id,
        }
    }

    fn integer(value: i128) -> Operand {
        Operand::Constant(TypedConstant {
            type_id: I8,
            value: Constant::Integer(value),
        })
    }

    fn binary(id: u32, result: Value, op: BinaryOp, left: Operand, right: Operand) -> Instruction {
        Instruction {
            id: InstructionId::new(id),
            results: vec![result],
            kind: InstructionKind::Binary { op, left, right },
        }
    }

    fn module(blocks: Vec<Block>, parameters: Vec<Value>) -> crate::ir::Module {
        crate::ir::Module {
            name: "cse-test".into(),
            types: vec![
                Type {
                    id: I8,
                    kind: TypeKind::Integer { bits: 8 },
                },
                Type {
                    id: PTR,
                    kind: TypeKind::Pointer {
                        address_space: crate::ir::AddressSpace::Generic,
                    },
                },
            ],
            globals: Vec::new(),
            functions: vec![Function {
                id: FunctionId::new(0),
                name: "main".into(),
                signature: Signature {
                    result: I8,
                    parameters: parameters
                        .iter()
                        .map(|parameter| parameter.type_id)
                        .collect(),
                    variadic: false,
                    calling_convention: CallingConvention::C,
                },
                linkage: Linkage::Internal,
                attributes: Vec::new(),
                parameters,
                blocks,
            }],
        }
    }

    #[test]
    fn removes_a_duplicate_expression_and_rewrites_the_terminator() {
        let left = value(10, I8);
        let right = value(11, I8);
        let first = value(0, I8);
        let repeated = value(1, I8);
        let mut module = module(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    binary(
                        0,
                        first.clone(),
                        BinaryOp::Add,
                        Operand::Value(left.id),
                        Operand::Value(right.id),
                    ),
                    binary(
                        1,
                        repeated.clone(),
                        BinaryOp::Add,
                        Operand::Value(left.id),
                        Operand::Value(right.id),
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Value(repeated.id))),
            }],
            vec![left, right],
        );

        let outcome = CommonSubexpressionElimination::new()
            .run(&mut module.functions[0])
            .unwrap();

        assert!(outcome.changed_ir());
        assert_eq!(outcome.preserved_analyses(), PreservedAnalyses::None);
        assert_eq!(module.functions[0].blocks[0].instructions.len(), 1);
        assert_eq!(
            module.functions[0].blocks[0].terminator,
            Terminator::Return(Some(Operand::Value(first.id)))
        );
    }

    #[test]
    fn keeps_reversed_operands_as_distinct_expressions() {
        let left = value(10, I8);
        let right = value(11, I8);
        let mut module = module(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    binary(
                        0,
                        value(0, I8),
                        BinaryOp::Add,
                        Operand::Value(left.id),
                        Operand::Value(right.id),
                    ),
                    binary(
                        1,
                        value(1, I8),
                        BinaryOp::Add,
                        Operand::Value(right.id),
                        Operand::Value(left.id),
                    ),
                ],
                terminator: Terminator::Return(Some(integer(0))),
            }],
            vec![left, right],
        );

        let outcome = CommonSubexpressionElimination::new()
            .run(&mut module.functions[0])
            .unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(outcome.preserved_analyses(), PreservedAnalyses::All);
        assert_eq!(module.functions[0].blocks[0].instructions.len(), 2);
    }

    #[test]
    fn preserves_trapping_divides_and_memory_or_effectful_operations() {
        let numerator = value(10, I8);
        let denominator = value(11, I8);
        let address = value(12, PTR);
        let mut module = module(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    binary(
                        0,
                        value(0, I8),
                        BinaryOp::SignedDivide,
                        Operand::Value(numerator.id),
                        Operand::Value(denominator.id),
                    ),
                    binary(
                        1,
                        value(1, I8),
                        BinaryOp::SignedDivide,
                        Operand::Value(numerator.id),
                        Operand::Value(denominator.id),
                    ),
                    Instruction {
                        id: InstructionId::new(2),
                        results: vec![value(2, I8)],
                        kind: InstructionKind::Load {
                            address: Operand::Value(address.id),
                            alignment: 1,
                            volatile: false,
                        },
                    },
                    Instruction {
                        id: InstructionId::new(3),
                        results: vec![value(3, I8)],
                        kind: InstructionKind::Load {
                            address: Operand::Value(address.id),
                            alignment: 1,
                            volatile: false,
                        },
                    },
                    Instruction {
                        id: InstructionId::new(4),
                        results: vec![value(4, I8)],
                        kind: InstructionKind::Call {
                            callee: Callee::Direct(FunctionId::new(9)),
                            arguments: Vec::new(),
                            effects: Effects {
                                memory: MemoryEffects::Read,
                                may_trap: false,
                                observable: false,
                            },
                        },
                    },
                    Instruction {
                        id: InstructionId::new(5),
                        results: vec![value(5, I8)],
                        kind: InstructionKind::Call {
                            callee: Callee::Direct(FunctionId::new(9)),
                            arguments: Vec::new(),
                            effects: Effects {
                                memory: MemoryEffects::Read,
                                may_trap: false,
                                observable: false,
                            },
                        },
                    },
                ],
                terminator: Terminator::Return(Some(integer(0))),
            }],
            vec![numerator, denominator, address],
        );

        let outcome = CommonSubexpressionElimination::new()
            .run(&mut module.functions[0])
            .unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(module.functions[0].blocks[0].instructions.len(), 6);
    }

    #[test]
    fn keeps_identical_stack_allocations_distinct() {
        let mut module = module(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    Instruction {
                        id: InstructionId::new(0),
                        results: vec![value(0, PTR)],
                        kind: InstructionKind::StackAlloc {
                            size: 8,
                            alignment: 4,
                            address_space: crate::ir::AddressSpace::Generic,
                        },
                    },
                    Instruction {
                        id: InstructionId::new(1),
                        results: vec![value(1, PTR)],
                        kind: InstructionKind::StackAlloc {
                            size: 8,
                            alignment: 4,
                            address_space: crate::ir::AddressSpace::Generic,
                        },
                    },
                ],
                terminator: Terminator::Return(Some(integer(0))),
            }],
            Vec::new(),
        );

        let outcome = CommonSubexpressionElimination::new()
            .run(&mut module.functions[0])
            .unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(module.functions[0].blocks[0].instructions.len(), 2);
    }
}
