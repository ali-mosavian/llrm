//! Deterministic removal of blocks unreachable from a function's entry.
//!
//! The pass changes only block membership and phi predecessor lists.  It does
//! not simplify control flow or delete instructions within a reachable block.

use std::collections::{BTreeMap, BTreeSet};

use crate::old::ir::{Block, BlockId, Function, InstructionKind, Terminator};

use super::{FunctionPass, PassFailure, PassOutcome, PreservedAnalyses};

/// Removes blocks unreachable from the function's first block.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnreachableBlockElimination;

impl UnreachableBlockElimination {
    /// Creates an unreachable-block elimination pass.
    pub const fn new() -> Self {
        Self
    }
}

impl FunctionPass for UnreachableBlockElimination {
    fn name(&self) -> &'static str {
        "unreachable-block-elimination"
    }

    fn run(&mut self, function: &mut Function) -> Result<PassOutcome, PassFailure> {
        let Some(entry) = function.blocks.first().map(|block| block.id) else {
            return Ok(PassOutcome::unchanged());
        };
        let blocks = index_blocks(function)?;
        let reachable = reachable_blocks(entry, &blocks)?;
        let removed = blocks
            .keys()
            .filter(|block| !reachable.contains(block))
            .copied()
            .collect::<BTreeSet<_>>();

        let original_count = function.blocks.len();
        function
            .blocks
            .retain(|block| reachable.contains(&block.id));
        let mut changed = function.blocks.len() != original_count;

        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                let InstructionKind::Phi { incoming } = &mut instruction.kind else {
                    continue;
                };
                let original_count = incoming.len();
                incoming.retain(|edge| !removed.contains(&edge.predecessor));
                changed |= incoming.len() != original_count;
            }
        }

        Ok(if changed {
            PassOutcome::changed(PreservedAnalyses::None)
        } else {
            PassOutcome::unchanged()
        })
    }
}

fn index_blocks(function: &Function) -> Result<BTreeMap<BlockId, &Block>, PassFailure> {
    let mut blocks = BTreeMap::new();
    for block in &function.blocks {
        if blocks.insert(block.id, block).is_some() {
            return Err(PassFailure::new(format!(
                "function has duplicate block id {}",
                block.id
            )));
        }
    }
    Ok(blocks)
}

fn reachable_blocks(
    entry: BlockId,
    blocks: &BTreeMap<BlockId, &Block>,
) -> Result<BTreeSet<BlockId>, PassFailure> {
    let mut pending = BTreeSet::from([entry]);
    let mut reachable = BTreeSet::new();

    while let Some(block_id) = pending.first().copied() {
        pending.remove(&block_id);
        if !reachable.insert(block_id) {
            continue;
        }
        let block = blocks.get(&block_id).ok_or_else(|| {
            PassFailure::new(format!(
                "reachable block {block_id} is absent from the function"
            ))
        })?;
        for target in terminator_targets(&block.terminator) {
            if !blocks.contains_key(&target) {
                return Err(PassFailure::new(format!(
                    "reachable block {block_id} targets unknown block {target}"
                )));
            }
            if !reachable.contains(&target) {
                pending.insert(target);
            }
        }
    }

    Ok(reachable)
}

fn terminator_targets(terminator: &Terminator) -> BTreeSet<BlockId> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::old::ir::{
        Block, CallingConvention, Constant, FunctionId, Instruction, InstructionId, Linkage,
        Operand, PhiIncoming, Signature, TypeId, TypedConstant, Value, ValueId,
    };

    const I16: TypeId = TypeId::new(0);

    fn function(blocks: Vec<Block>) -> Function {
        Function {
            id: FunctionId::new(0),
            name: "unreachable".to_owned(),
            signature: Signature {
                result: I16,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: CallingConvention::FarPascal,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks,
        }
    }

    fn integer(value: i128) -> Operand {
        Operand::Constant(TypedConstant {
            type_id: I16,
            value: Constant::Integer(value),
        })
    }

    fn phi(id: u32, incoming: Vec<(u32, i128)>) -> Instruction {
        Instruction {
            id: InstructionId::new(id),
            results: vec![Value {
                id: ValueId::new(id),
                type_id: I16,
            }],
            kind: InstructionKind::Phi {
                incoming: incoming
                    .into_iter()
                    .map(|(predecessor, value)| PhiIncoming {
                        predecessor: BlockId::new(predecessor),
                        value: integer(value),
                    })
                    .collect(),
            },
        }
    }

    #[test]
    fn removes_unreachable_blocks_repairs_phis_and_keeps_original_order() {
        let mut function = function(vec![
            Block {
                id: BlockId::new(4),
                instructions: Vec::new(),
                terminator: Terminator::Jump(BlockId::new(6)),
            },
            Block {
                id: BlockId::new(5),
                instructions: Vec::new(),
                terminator: Terminator::Jump(BlockId::new(6)),
            },
            Block {
                id: BlockId::new(6),
                instructions: vec![phi(0, vec![(4, 1), (5, 2)])],
                terminator: Terminator::Return(Some(integer(0))),
            },
        ]);

        let outcome = UnreachableBlockElimination::new()
            .run(&mut function)
            .expect("reachable CFG is well-formed");

        assert!(outcome.changed_ir());
        assert_eq!(outcome.preserved_analyses(), PreservedAnalyses::None);
        assert_eq!(
            function
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            vec![BlockId::new(4), BlockId::new(6)]
        );
        assert!(matches!(
            &function.blocks[1].instructions[0].kind,
            InstructionKind::Phi { .. }
        ));
        let InstructionKind::Phi { incoming } = &function.blocks[1].instructions[0].kind else {
            return;
        };
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].predecessor, BlockId::new(4));
    }

    #[test]
    fn retains_reachable_loops() {
        let mut function = function(vec![
            Block {
                id: BlockId::new(10),
                instructions: Vec::new(),
                terminator: Terminator::Jump(BlockId::new(11)),
            },
            Block {
                id: BlockId::new(11),
                instructions: Vec::new(),
                terminator: Terminator::Jump(BlockId::new(10)),
            },
            Block {
                id: BlockId::new(12),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(integer(0))),
            },
        ]);

        let outcome = UnreachableBlockElimination::new()
            .run(&mut function)
            .expect("loop targets are present");

        assert!(outcome.changed_ir());
        assert_eq!(function.blocks.len(), 2);
        assert_eq!(function.blocks[0].id, BlockId::new(10));
        assert_eq!(function.blocks[1].id, BlockId::new(11));
    }

    #[test]
    fn refuses_unknown_targets_from_reachable_blocks_without_mutation() {
        let mut function = function(vec![
            Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Jump(BlockId::new(9)),
            },
            Block {
                id: BlockId::new(1),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(integer(0))),
            },
        ]);
        let original = function.clone();

        let error = UnreachableBlockElimination::new()
            .run(&mut function)
            .expect_err("reachable unknown targets must be refused");

        assert!(error.message().contains("targets unknown block 9"));
        assert_eq!(function, original);
    }

    #[test]
    fn leaves_an_empty_function_unchanged() {
        let mut function = function(Vec::new());

        let outcome = UnreachableBlockElimination::new()
            .run(&mut function)
            .expect("empty functions need no CFG traversal");

        assert!(!outcome.changed_ir());
        assert_eq!(outcome.preserved_analyses(), PreservedAnalyses::All);
    }
}
