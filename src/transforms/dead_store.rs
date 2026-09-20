//! Conservative, same-basic-block dead-store elimination.
//!
//! A store is removed only when a later store overwrites the exact same
//! constant global address with the same stored value type before any
//! instruction that can observe memory.
//! This deliberately does not infer aliases, inspect control flow, or look
//! through computed addresses.

use std::collections::BTreeMap;

use crate::ir::{
    Constant, Function, GlobalId, Instruction, InstructionKind, MemoryEffects, Operand, TypeId,
    ValueId,
};

use super::{FunctionPass, PassFailure, PassOutcome, PreservedAnalyses};

/// Removes overwritten direct global stores within a single basic block.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeadStoreElimination;

impl DeadStoreElimination {
    /// Creates a conservative dead-store elimination pass.
    pub const fn new() -> Self {
        Self
    }
}

impl FunctionPass for DeadStoreElimination {
    fn name(&self) -> &'static str {
        "dead-store-elimination"
    }

    fn run(&mut self, function: &mut Function) -> Result<PassOutcome, PassFailure> {
        let mut changed = false;
        let value_types = value_types(function);

        for block in &mut function.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut retained = Vec::with_capacity(instructions.len());
            let mut overwritten = Vec::with_capacity(instructions.len());
            let mut latest_store = BTreeMap::new();

            for instruction in instructions {
                if let Some(target) = direct_store_target(&instruction, &value_types) {
                    if let Some(previous) = latest_store.insert(target, retained.len()) {
                        overwritten[previous] = true;
                        changed = true;
                    }
                } else if is_memory_barrier(&instruction) {
                    latest_store.clear();
                }

                retained.push(instruction);
                overwritten.push(false);
            }

            block.instructions = retained
                .into_iter()
                .zip(overwritten)
                .filter_map(|(instruction, overwritten)| (!overwritten).then_some(instruction))
                .collect();
        }

        Ok(if changed {
            PassOutcome::changed(PreservedAnalyses::None)
        } else {
            PassOutcome::unchanged()
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DirectGlobalAddress {
    type_id: TypeId,
    global: GlobalId,
    addend: i64,
    value_type: TypeId,
}

fn direct_store_target(
    instruction: &Instruction,
    value_types: &BTreeMap<ValueId, TypeId>,
) -> Option<DirectGlobalAddress> {
    let InstructionKind::Store {
        address,
        value,
        volatile: false,
        ..
    } = &instruction.kind
    else {
        return None;
    };
    let Operand::Constant(address) = address else {
        return None;
    };
    let Constant::GlobalAddress { global, addend } = address.value else {
        return None;
    };
    Some(DirectGlobalAddress {
        type_id: address.type_id,
        global,
        addend,
        value_type: operand_type(value, value_types)?,
    })
}

fn value_types(function: &Function) -> BTreeMap<ValueId, TypeId> {
    let mut value_types = BTreeMap::new();
    for value in function.parameters.iter().chain(
        function
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .flat_map(|instruction| instruction.results.iter()),
    ) {
        value_types.entry(value.id).or_insert(value.type_id);
    }
    value_types
}

fn operand_type(operand: &Operand, value_types: &BTreeMap<ValueId, TypeId>) -> Option<TypeId> {
    match operand {
        Operand::Constant(constant) => Some(constant.type_id),
        Operand::Value(value) => value_types.get(value).copied(),
    }
}

fn is_memory_barrier(instruction: &Instruction) -> bool {
    match &instruction.kind {
        InstructionKind::Load { .. } => true,
        InstructionKind::Store { .. } => true,
        InstructionKind::Call { effects, .. } => !matches!(effects.memory, MemoryEffects::None),
        InstructionKind::Intrinsic { intrinsic, .. } => {
            !matches!(intrinsic.effects().memory, MemoryEffects::None)
        }
        InstructionKind::Phi { .. }
        | InstructionKind::Unary { .. }
        | InstructionKind::Binary { .. }
        | InstructionKind::Compare { .. }
        | InstructionKind::Cast { .. }
        | InstructionKind::GetElementPointer { .. }
        | InstructionKind::Select { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        Block, BlockId, Callee, CallingConvention, Constant, Effects, Function,
        FunctionId, InstructionId, Linkage, Signature, Terminator, TypedConstant, Value, ValueId,
    };

    const I8: TypeId = TypeId::new(0);
    const PTR: TypeId = TypeId::new(1);
    const I32: TypeId = TypeId::new(2);

    fn integer(value: i128) -> Operand {
        Operand::Constant(TypedConstant {
            type_id: I8,
            value: Constant::Integer(value),
        })
    }

    fn global_address(addend: i64) -> Operand {
        Operand::Constant(TypedConstant {
            type_id: PTR,
            value: Constant::GlobalAddress {
                global: GlobalId::new(0),
                addend,
            },
        })
    }

    fn store(id: u32, address: Operand, value: i128) -> Instruction {
        store_constant(
            id,
            address,
            TypedConstant {
                type_id: I8,
                value: Constant::Integer(value),
            },
        )
    }

    fn store_constant(id: u32, address: Operand, value: TypedConstant) -> Instruction {
        Instruction {
            id: InstructionId::new(id),
            results: Vec::new(),
            kind: InstructionKind::Store {
                address,
                value: Operand::Constant(value),
                alignment: 1,
                volatile: false,
            },
        }
    }

    fn function(instructions: Vec<Instruction>, parameters: Vec<Value>) -> Function {
        Function {
            id: FunctionId::new(0),
            name: "dead-store".to_owned(),
            signature: Signature {
                result: I8,
                parameters: parameters.iter().map(|value| value.type_id).collect(),
                variadic: false,
                calling_convention: CallingConvention::Basic,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters,
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions,
                terminator: Terminator::Return(Some(integer(0))),
            }],
        }
    }

    #[test]
    fn removes_an_overwritten_direct_global_store() {
        let mut function = function(
            vec![store(0, global_address(0), 1), store(1, global_address(0), 2)],
            Vec::new(),
        );

        let outcome = DeadStoreElimination::new().run(&mut function).unwrap();

        assert!(outcome.changed_ir());
        assert_eq!(outcome.preserved_analyses(), PreservedAnalyses::None);
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![InstructionId::new(1)]
        );
    }

    #[test]
    fn preserves_stores_across_memory_observation_and_unknown_addresses() {
        let unknown_address = Value {
            id: ValueId::new(0),
            type_id: PTR,
        };
        let load = Instruction {
            id: InstructionId::new(1),
            results: vec![Value {
                id: ValueId::new(1),
                type_id: I8,
            }],
            kind: InstructionKind::Load {
                address: global_address(0),
                alignment: 1,
                volatile: false,
            },
        };
        let call = Instruction {
            id: InstructionId::new(3),
            results: Vec::new(),
            kind: InstructionKind::Call {
                callee: Callee::Direct(FunctionId::new(1)),
                arguments: Vec::new(),
                effects: Effects {
                    memory: MemoryEffects::Read,
                    may_trap: false,
                    observable: false,
                },
            },
        };
        let mut function = function(
            vec![
                store(0, global_address(0), 1),
                load,
                store(2, global_address(0), 2),
                call,
                store(4, global_address(0), 3),
                store(5, Operand::Value(unknown_address.id), 4),
                store(6, global_address(0), 5),
            ],
            vec![unknown_address],
        );

        let outcome = DeadStoreElimination::new().run(&mut function).unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            (0..=6).map(InstructionId::new).collect::<Vec<_>>()
        );
    }

    #[test]
    fn keeps_direct_global_stores_with_distinct_addends() {
        let mut function = function(
            vec![store(0, global_address(0), 1), store(1, global_address(1), 2)],
            Vec::new(),
        );

        let outcome = DeadStoreElimination::new().run(&mut function).unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(function.blocks[0].instructions.len(), 2);
    }

    #[test]
    fn keeps_direct_global_stores_with_distinct_value_types() {
        let mut function = function(
            vec![
                store(0, global_address(0), 1),
                store_constant(
                    1,
                    global_address(0),
                    TypedConstant {
                        type_id: I32,
                        value: Constant::Integer(2),
                    },
                ),
            ],
            Vec::new(),
        );

        let outcome = DeadStoreElimination::new().run(&mut function).unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(function.blocks[0].instructions.len(), 2);
    }

    #[test]
    fn treats_an_unresolved_store_value_type_as_a_barrier() {
        let mut function = function(
            vec![
                store(0, global_address(0), 1),
                Instruction {
                    id: InstructionId::new(1),
                    results: Vec::new(),
                    kind: InstructionKind::Store {
                        address: global_address(0),
                        value: Operand::Value(ValueId::new(99)),
                        alignment: 1,
                        volatile: false,
                    },
                },
                store(2, global_address(0), 2),
            ],
            Vec::new(),
        );

        let outcome = DeadStoreElimination::new().run(&mut function).unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(function.blocks[0].instructions.len(), 3);
    }

    #[test]
    fn does_not_eliminate_stores_across_blocks() {
        let mut function = function(vec![store(0, global_address(0), 1)], Vec::new());
        function.blocks[0].terminator = Terminator::Jump(BlockId::new(1));
        function.blocks.push(Block {
            id: BlockId::new(1),
            instructions: vec![store(1, global_address(0), 2)],
            terminator: Terminator::Return(Some(integer(0))),
        });

        let outcome = DeadStoreElimination::new().run(&mut function).unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(function.blocks[0].instructions.len(), 1);
        assert_eq!(function.blocks[1].instructions.len(), 1);
    }
}
