//! Exact simplification of constant control-flow terminators.
//!
//! This pass only replaces a branch or switch whose selector is already a
//! typed integer constant.  It repairs the affected phi edges, but does not
//! remove blocks or otherwise reshape the CFG.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::old::ir::{
    BlockId, Constant, Function, InstructionKind, Module, Operand, Terminator, TypeId, TypeKind,
};

use super::{FunctionPass, PassFailure, PassOutcome, PreservedAnalyses};

/// A construction error for [`SimplifyBranches`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BranchSimplifyError {
    /// One type identity was declared more than once.
    DuplicateType(TypeId),
    /// An integer width cannot be represented by the portable integer oracle.
    InvalidIntegerWidth { type_id: TypeId, bits: u16 },
}

impl fmt::Display for BranchSimplifyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateType(type_id) => write!(formatter, "duplicate type id {type_id}"),
            Self::InvalidIntegerWidth { type_id, bits } => {
                write!(
                    formatter,
                    "integer type {type_id} has unsupported width {bits}"
                )
            }
        }
    }
}

impl std::error::Error for BranchSimplifyError {}

/// Replaces integer-constant branches and switches with jumps.
///
/// The pass owns a deterministic copy of the module's integer type widths so
/// it can implement [`FunctionPass`] without borrowing the enclosing module.
#[derive(Clone, Debug)]
pub struct SimplifyBranches {
    integer_widths: BTreeMap<TypeId, u16>,
}

impl SimplifyBranches {
    /// Builds a simplifier from the module's declared integer types.
    pub fn new(module: &Module) -> Result<Self, BranchSimplifyError> {
        let mut integer_widths = BTreeMap::new();
        let mut declared = BTreeSet::new();

        for type_ in &module.types {
            if !declared.insert(type_.id) {
                return Err(BranchSimplifyError::DuplicateType(type_.id));
            }
            if let TypeKind::Integer { bits } = &type_.kind {
                if !(1..=128).contains(bits) {
                    return Err(BranchSimplifyError::InvalidIntegerWidth {
                        type_id: type_.id,
                        bits: *bits,
                    });
                }
                integer_widths.insert(type_.id, *bits);
            }
        }

        Ok(Self { integer_widths })
    }

    fn integer_constant(&self, operand: &Operand) -> Option<IntegerConstant> {
        let Operand::Constant(constant) = operand else {
            return None;
        };
        let Constant::Integer(value) = &constant.value else {
            return None;
        };
        let bits = self.integer_widths.get(&constant.type_id).copied()?;
        Some(IntegerConstant {
            bits,
            raw: (*value as u128) & integer_mask(bits),
        })
    }

    fn simplify(&self, terminator: &Terminator) -> Option<BlockId> {
        match terminator {
            Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                let condition = self.integer_constant(condition)?;
                if condition.bits != 1 {
                    return None;
                }
                Some(if condition.raw == 0 {
                    *else_block
                } else {
                    *then_block
                })
            }
            Terminator::Switch {
                selector,
                cases,
                default,
            } => {
                let selector = self.integer_constant(selector)?;
                for (value, target) in cases {
                    if ((*value as u128) & integer_mask(selector.bits)) == selector.raw {
                        return Some(*target);
                    }
                }
                Some(*default)
            }
            Terminator::Jump(_) | Terminator::Return(_) | Terminator::Unreachable => None,
        }
    }
}

impl FunctionPass for SimplifyBranches {
    fn name(&self) -> &'static str {
        "simplify-branches"
    }

    fn run(&mut self, function: &mut Function) -> Result<PassOutcome, PassFailure> {
        let changes = function
            .blocks
            .iter()
            .enumerate()
            .filter_map(|(index, block)| {
                let target = self.simplify(&block.terminator)?;
                let old_successors = successors(&block.terminator);
                let removed_successors = old_successors
                    .difference(&BTreeSet::from([target]))
                    .copied()
                    .collect();
                Some(TerminatorChange {
                    index,
                    source: block.id,
                    target,
                    removed_successors,
                })
            })
            .collect::<Vec<_>>();

        if changes.is_empty() {
            return Ok(PassOutcome::unchanged());
        }

        for change in changes {
            function.blocks[change.index].terminator = Terminator::Jump(change.target);
            for target in change.removed_successors {
                remove_phi_predecessor(function, target, change.source);
            }
        }

        Ok(PassOutcome::changed(PreservedAnalyses::None))
    }
}

#[derive(Debug)]
struct TerminatorChange {
    index: usize,
    source: BlockId,
    target: BlockId,
    removed_successors: BTreeSet<BlockId>,
}

#[derive(Clone, Copy)]
struct IntegerConstant {
    bits: u16,
    raw: u128,
}

fn integer_mask(bits: u16) -> u128 {
    match bits {
        1..=127 => (1u128 << bits) - 1,
        128 => u128::MAX,
        _ => unreachable!("SimplifyBranches validates integer widths at construction"),
    }
}

fn successors(terminator: &Terminator) -> BTreeSet<BlockId> {
    match terminator {
        Terminator::Jump(target) => BTreeSet::from([*target]),
        Terminator::Branch {
            then_block,
            else_block,
            ..
        } => BTreeSet::from([*then_block, *else_block]),
        Terminator::Switch { cases, default, .. } => cases
            .iter()
            .map(|(_, target)| *target)
            .chain(std::iter::once(*default))
            .collect(),
        Terminator::Return(_) | Terminator::Unreachable => BTreeSet::new(),
    }
}

fn remove_phi_predecessor(function: &mut Function, target: BlockId, predecessor: BlockId) {
    for block in function
        .blocks
        .iter_mut()
        .filter(|block| block.id == target)
    {
        for instruction in &mut block.instructions {
            if let InstructionKind::Phi { incoming } = &mut instruction.kind {
                incoming.retain(|source| source.predecessor != predecessor);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::old::ir::{
        Block, CallingConvention, Function, FunctionId, Instruction, InstructionId, Linkage,
        PhiIncoming, Signature, Type, TypedConstant, Value, ValueId,
    };

    const I1: TypeId = TypeId::new(0);
    const I8: TypeId = TypeId::new(1);

    fn integer(type_id: TypeId, value: i128) -> Operand {
        Operand::Constant(TypedConstant {
            type_id,
            value: Constant::Integer(value),
        })
    }

    fn module(blocks: Vec<Block>, parameters: Vec<Value>) -> Module {
        Module {
            name: "branch-simplify-test".into(),
            types: vec![
                Type {
                    id: I1,
                    kind: TypeKind::Integer { bits: 1 },
                },
                Type {
                    id: I8,
                    kind: TypeKind::Integer { bits: 8 },
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

    fn phi(id: u32, incoming: Vec<(BlockId, Operand)>) -> Instruction {
        Instruction {
            id: InstructionId::new(id),
            results: vec![Value {
                id: ValueId::new(id),
                type_id: I8,
            }],
            kind: InstructionKind::Phi {
                incoming: incoming
                    .into_iter()
                    .map(|(predecessor, value)| PhiIncoming { predecessor, value })
                    .collect(),
            },
        }
    }

    #[test]
    fn repairs_only_the_removed_branch_phi_edge() {
        let mut module = module(
            vec![
                Block {
                    id: BlockId::new(0),
                    instructions: Vec::new(),
                    terminator: Terminator::Branch {
                        condition: integer(I1, 1),
                        then_block: BlockId::new(1),
                        else_block: BlockId::new(2),
                    },
                },
                Block {
                    id: BlockId::new(1),
                    instructions: vec![phi(0, vec![(BlockId::new(0), integer(I8, 1))])],
                    terminator: Terminator::Return(Some(integer(I8, 1))),
                },
                Block {
                    id: BlockId::new(2),
                    instructions: vec![phi(1, vec![(BlockId::new(0), integer(I8, 2))])],
                    terminator: Terminator::Return(Some(integer(I8, 2))),
                },
            ],
            Vec::new(),
        );

        let outcome = SimplifyBranches::new(&module)
            .unwrap()
            .run(&mut module.functions[0])
            .unwrap();

        assert!(outcome.changed_ir());
        assert_eq!(
            module.functions[0].blocks[0].terminator,
            Terminator::Jump(BlockId::new(1))
        );
        let InstructionKind::Phi { incoming } = &module.functions[0].blocks[1].instructions[0].kind
        else {
            panic!("expected phi");
        };
        assert_eq!(incoming.len(), 1);
        let InstructionKind::Phi { incoming } = &module.functions[0].blocks[2].instructions[0].kind
        else {
            panic!("expected phi");
        };
        assert!(incoming.is_empty());
    }

    #[test]
    fn switch_uses_first_normalized_match_and_keeps_duplicate_target_edge() {
        let mut module = module(
            vec![
                Block {
                    id: BlockId::new(0),
                    instructions: Vec::new(),
                    terminator: Terminator::Switch {
                        selector: integer(I8, -1),
                        cases: vec![
                            (255, BlockId::new(1)),
                            (-1, BlockId::new(2)),
                            (7, BlockId::new(1)),
                        ],
                        default: BlockId::new(3),
                    },
                },
                Block {
                    id: BlockId::new(1),
                    instructions: vec![phi(0, vec![(BlockId::new(0), integer(I8, 1))])],
                    terminator: Terminator::Return(Some(integer(I8, 1))),
                },
                Block {
                    id: BlockId::new(2),
                    instructions: vec![phi(1, vec![(BlockId::new(0), integer(I8, 2))])],
                    terminator: Terminator::Return(Some(integer(I8, 2))),
                },
                Block {
                    id: BlockId::new(3),
                    instructions: vec![phi(2, vec![(BlockId::new(0), integer(I8, 3))])],
                    terminator: Terminator::Return(Some(integer(I8, 3))),
                },
            ],
            Vec::new(),
        );

        SimplifyBranches::new(&module)
            .unwrap()
            .run(&mut module.functions[0])
            .unwrap();

        assert_eq!(
            module.functions[0].blocks[0].terminator,
            Terminator::Jump(BlockId::new(1))
        );
        for index in [2, 3] {
            let InstructionKind::Phi { incoming } =
                &module.functions[0].blocks[index].instructions[0].kind
            else {
                panic!("expected phi");
            };
            assert!(incoming.is_empty());
        }
        let InstructionKind::Phi { incoming } = &module.functions[0].blocks[1].instructions[0].kind
        else {
            panic!("expected phi");
        };
        assert_eq!(incoming.len(), 1);
    }

    #[test]
    fn switch_uses_default_when_no_case_matches() {
        let mut module = module(
            vec![
                Block {
                    id: BlockId::new(0),
                    instructions: Vec::new(),
                    terminator: Terminator::Switch {
                        selector: integer(I8, 5),
                        cases: vec![(1, BlockId::new(1))],
                        default: BlockId::new(2),
                    },
                },
                Block {
                    id: BlockId::new(1),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(Some(integer(I8, 1))),
                },
                Block {
                    id: BlockId::new(2),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(Some(integer(I8, 2))),
                },
            ],
            Vec::new(),
        );

        SimplifyBranches::new(&module)
            .unwrap()
            .run(&mut module.functions[0])
            .unwrap();

        assert_eq!(
            module.functions[0].blocks[0].terminator,
            Terminator::Jump(BlockId::new(2))
        );
    }

    #[test]
    fn leaves_dynamic_control_flow_unchanged() {
        let selector = Value {
            id: ValueId::new(0),
            type_id: I8,
        };
        let mut module = module(
            vec![
                Block {
                    id: BlockId::new(0),
                    instructions: Vec::new(),
                    terminator: Terminator::Switch {
                        selector: Operand::Value(selector.id),
                        cases: vec![(1, BlockId::new(1))],
                        default: BlockId::new(2),
                    },
                },
                Block {
                    id: BlockId::new(1),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(Some(integer(I8, 1))),
                },
                Block {
                    id: BlockId::new(2),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(Some(integer(I8, 2))),
                },
            ],
            vec![selector],
        );
        let original = module.functions[0].clone();

        let outcome = SimplifyBranches::new(&module)
            .unwrap()
            .run(&mut module.functions[0])
            .unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(module.functions[0], original);
    }
}
