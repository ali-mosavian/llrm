//! Deterministic definition and use indexing for one IR function.

use std::collections::BTreeMap;
use std::fmt;

use crate::ir::{
    BlockId, Callee, Function, Instruction, InstructionId, InstructionKind, Operand, Terminator,
    ValueId,
};

/// The unique definition of an SSA value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Definition {
    Parameter {
        index: usize,
    },
    Instruction {
        block: BlockId,
        instruction: InstructionId,
        result_index: usize,
    },
}

/// One read of an SSA value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Use {
    InstructionOperand {
        block: BlockId,
        instruction: InstructionId,
        operand_index: usize,
    },
    PhiIncoming {
        block: BlockId,
        instruction: InstructionId,
        incoming_index: usize,
        predecessor: BlockId,
    },
    TerminatorOperand {
        block: BlockId,
        operand_index: usize,
    },
}

/// Definition and use lists in deterministic function order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefUse {
    definitions: BTreeMap<ValueId, Definition>,
    uses: BTreeMap<ValueId, Vec<Use>>,
}

impl DefUse {
    pub fn analyze(function: &Function) -> Result<Self, DefUseError> {
        let mut analysis = Self {
            definitions: BTreeMap::new(),
            uses: BTreeMap::new(),
        };
        for (index, parameter) in function.parameters.iter().enumerate() {
            analysis.define(parameter.id, Definition::Parameter { index })?;
        }
        for block in &function.blocks {
            for instruction in &block.instructions {
                for (result_index, result) in instruction.results.iter().enumerate() {
                    analysis.define(
                        result.id,
                        Definition::Instruction {
                            block: block.id,
                            instruction: instruction.id,
                            result_index,
                        },
                    )?;
                }
                analysis.collect_instruction_uses(block.id, instruction);
            }
            analysis.collect_terminator_uses(block.id, &block.terminator);
        }
        if let Some(value) = analysis
            .uses
            .keys()
            .find(|value| !analysis.definitions.contains_key(value))
        {
            return Err(DefUseError::UnknownValue(*value));
        }
        Ok(analysis)
    }

    pub fn definition(&self, value: ValueId) -> Option<Definition> {
        self.definitions.get(&value).copied()
    }

    pub fn uses(&self, value: ValueId) -> &[Use] {
        self.uses.get(&value).map(Vec::as_slice).unwrap_or_default()
    }

    pub fn definitions(&self) -> &BTreeMap<ValueId, Definition> {
        &self.definitions
    }

    fn define(&mut self, value: ValueId, definition: Definition) -> Result<(), DefUseError> {
        if self.definitions.insert(value, definition).is_some() {
            Err(DefUseError::DuplicateDefinition(value))
        } else {
            Ok(())
        }
    }

    fn add_use(&mut self, operand: &Operand, use_: Use) {
        if let Operand::Value(value) = operand {
            self.uses.entry(*value).or_default().push(use_);
        }
    }

    fn collect_instruction_uses(&mut self, block: BlockId, instruction: &Instruction) {
        if let InstructionKind::Phi { incoming } = &instruction.kind {
            for (incoming_index, incoming) in incoming.iter().enumerate() {
                self.add_use(
                    &incoming.value,
                    Use::PhiIncoming {
                        block,
                        instruction: instruction.id,
                        incoming_index,
                        predecessor: incoming.predecessor,
                    },
                );
            }
            return;
        }
        for (operand_index, operand) in instruction_operands(&instruction.kind)
            .into_iter()
            .enumerate()
        {
            self.add_use(
                operand,
                Use::InstructionOperand {
                    block,
                    instruction: instruction.id,
                    operand_index,
                },
            );
        }
    }

    fn collect_terminator_uses(&mut self, block: BlockId, terminator: &Terminator) {
        for (operand_index, operand) in terminator_operands(terminator).into_iter().enumerate() {
            self.add_use(
                operand,
                Use::TerminatorOperand {
                    block,
                    operand_index,
                },
            );
        }
    }
}

fn instruction_operands(kind: &InstructionKind) -> Vec<&Operand> {
    match kind {
        InstructionKind::Phi { .. } | InstructionKind::StackAlloc { .. } => Vec::new(),
        InstructionKind::Unary { operand, .. }
        | InstructionKind::Cast { operand, .. }
        | InstructionKind::Load {
            address: operand, ..
        } => vec![operand],
        InstructionKind::Binary { left, right, .. }
        | InstructionKind::Compare { left, right, .. } => vec![left, right],
        InstructionKind::Store { address, value, .. } => vec![address, value],
        InstructionKind::ComposePointer { segment, offset } => vec![segment, offset],
        InstructionKind::GetElementPointer { base, indices } => {
            let mut operands = Vec::with_capacity(indices.len() + 1);
            operands.push(base);
            operands.extend(indices);
            operands
        }
        InstructionKind::Select {
            condition,
            then_value,
            else_value,
        } => vec![condition, then_value, else_value],
        InstructionKind::Call {
            callee, arguments, ..
        } => {
            let mut operands = Vec::with_capacity(arguments.len() + 1);
            if let Callee::Indirect(callee) = callee {
                operands.push(callee);
            }
            operands.extend(arguments);
            operands
        }
        InstructionKind::Intrinsic { arguments, .. } => arguments.iter().collect(),
    }
}

fn terminator_operands(terminator: &Terminator) -> Vec<&Operand> {
    match terminator {
        Terminator::Branch { condition, .. } => vec![condition],
        Terminator::Switch { selector, .. } => vec![selector],
        Terminator::Return(Some(value)) => vec![value],
        Terminator::Jump(_) | Terminator::Return(None) | Terminator::Unreachable => Vec::new(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefUseError {
    DuplicateDefinition(ValueId),
    UnknownValue(ValueId),
}

impl fmt::Display for DefUseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDefinition(value) => {
                write!(formatter, "value {value} has more than one definition")
            }
            Self::UnknownValue(value) => write!(formatter, "value {value} has no definition"),
        }
    }
}

impl std::error::Error for DefUseError {}

#[cfg(test)]
mod tests {
    use super::{DefUse, Definition, Use};
    use crate::ir::{
        Block, BlockId, CallingConvention, Function, FunctionId, Instruction, InstructionId,
        InstructionKind, Linkage, Operand, PhiIncoming, Signature, Terminator, TypeId, Value,
        ValueId,
    };

    #[test]
    fn indexes_phi_and_terminator_uses_in_function_order() {
        let function = Function {
            id: FunctionId::new(0),
            name: "loop".into(),
            signature: Signature {
                result: TypeId::new(0),
                parameters: vec![TypeId::new(0)],
                variadic: false,
                calling_convention: CallingConvention::FarPascal,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: vec![Value {
                id: ValueId::new(0),
                type_id: TypeId::new(0),
            }],
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: InstructionId::new(0),
                    results: vec![Value {
                        id: ValueId::new(1),
                        type_id: TypeId::new(0),
                    }],
                    kind: InstructionKind::Phi {
                        incoming: vec![PhiIncoming {
                            predecessor: BlockId::new(0),
                            value: Operand::Value(ValueId::new(0)),
                        }],
                    },
                }],
                terminator: Terminator::Branch {
                    condition: Operand::Value(ValueId::new(1)),
                    then_block: BlockId::new(0),
                    else_block: BlockId::new(0),
                },
            }],
        };

        let analysis = DefUse::analyze(&function).unwrap();

        assert_eq!(
            analysis.definition(ValueId::new(0)),
            Some(Definition::Parameter { index: 0 })
        );
        assert_eq!(
            analysis.uses(ValueId::new(0)),
            [Use::PhiIncoming {
                block: BlockId::new(0),
                instruction: InstructionId::new(0),
                incoming_index: 0,
                predecessor: BlockId::new(0),
            }]
        );
        assert_eq!(
            analysis.uses(ValueId::new(1)),
            [Use::TerminatorOperand {
                block: BlockId::new(0),
                operand_index: 0,
            }]
        );
    }

    #[test]
    fn indexes_compose_pointer_segment_before_offset() {
        let function = Function {
            id: FunctionId::new(0),
            name: "compose".into(),
            signature: Signature {
                result: TypeId::new(0),
                parameters: vec![TypeId::new(0), TypeId::new(0)],
                variadic: false,
                calling_convention: CallingConvention::FarPascal,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: vec![
                Value {
                    id: ValueId::new(0),
                    type_id: TypeId::new(0),
                },
                Value {
                    id: ValueId::new(1),
                    type_id: TypeId::new(0),
                },
            ],
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: InstructionId::new(0),
                    results: vec![Value {
                        id: ValueId::new(2),
                        type_id: TypeId::new(0),
                    }],
                    kind: InstructionKind::ComposePointer {
                        segment: Operand::Value(ValueId::new(0)),
                        offset: Operand::Value(ValueId::new(1)),
                    },
                }],
                terminator: Terminator::Return(None),
            }],
        };

        let analysis = DefUse::analyze(&function).unwrap();
        assert_eq!(
            analysis.uses(ValueId::new(0)),
            [Use::InstructionOperand {
                block: BlockId::new(0),
                instruction: InstructionId::new(0),
                operand_index: 0,
            }]
        );
        assert_eq!(
            analysis.uses(ValueId::new(1)),
            [Use::InstructionOperand {
                block: BlockId::new(0),
                instruction: InstructionId::new(0),
                operand_index: 1,
            }]
        );
    }
}
