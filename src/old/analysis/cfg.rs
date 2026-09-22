//! Deterministic control-flow edges for one IR function.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::old::ir::{BlockId, Function, Terminator};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlFlowGraph {
    entry: Option<BlockId>,
    edges: BTreeMap<BlockId, BlockEdges>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BlockEdges {
    predecessors: BTreeSet<BlockId>,
    successors: BTreeSet<BlockId>,
}

impl ControlFlowGraph {
    pub fn analyze(function: &Function) -> Result<Self, CfgError> {
        let mut edges = BTreeMap::new();
        for block in &function.blocks {
            if edges
                .insert(
                    block.id,
                    BlockEdges {
                        predecessors: BTreeSet::new(),
                        successors: BTreeSet::new(),
                    },
                )
                .is_some()
            {
                return Err(CfgError::DuplicateBlock(block.id));
            }
        }

        for block in &function.blocks {
            for_each_target(&block.terminator, |target| {
                if !edges.contains_key(&target) {
                    return Err(CfgError::UnknownTarget {
                        from: block.id,
                        target,
                    });
                }
                edges
                    .get_mut(&block.id)
                    .expect("source block was inserted above")
                    .successors
                    .insert(target);
                edges
                    .get_mut(&target)
                    .expect("target existence was checked above")
                    .predecessors
                    .insert(block.id);
                Ok(())
            })?;
        }

        Ok(Self {
            entry: function.blocks.first().map(|block| block.id),
            edges,
        })
    }

    pub const fn entry(&self) -> Option<BlockId> {
        self.entry
    }

    pub fn contains(&self, block: BlockId) -> bool {
        self.edges.contains_key(&block)
    }

    pub fn predecessors(&self, block: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.edges.get(&block).map(|edges| &edges.predecessors)
    }

    pub fn successors(&self, block: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.edges.get(&block).map(|edges| &edges.successors)
    }

    pub fn blocks(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.edges.keys().copied()
    }
}

fn for_each_target(
    terminator: &Terminator,
    mut visit: impl FnMut(BlockId) -> Result<(), CfgError>,
) -> Result<(), CfgError> {
    match terminator {
        Terminator::Jump(target) => visit(*target),
        Terminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            visit(*then_block)?;
            visit(*else_block)
        }
        Terminator::Switch { cases, default, .. } => {
            for (_, target) in cases {
                visit(*target)?;
            }
            visit(*default)
        }
        Terminator::Return(_) | Terminator::Unreachable => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CfgError {
    DuplicateBlock(BlockId),
    UnknownTarget { from: BlockId, target: BlockId },
}

impl fmt::Display for CfgError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateBlock(block) => write!(formatter, "duplicate block {block}"),
            Self::UnknownTarget { from, target } => {
                write!(formatter, "block {from} targets unknown block {target}")
            }
        }
    }
}

impl std::error::Error for CfgError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::ControlFlowGraph;
    use crate::old::ir::{
        Block, BlockId, CallingConvention, Constant, Function, FunctionId, Linkage, Operand,
        Signature, Terminator, TypeId, TypedConstant,
    };

    #[test]
    fn indexes_branch_edges_without_duplicate_predecessors() {
        let function = Function {
            id: FunctionId::new(0),
            name: "branch".into(),
            signature: Signature {
                result: TypeId::new(0),
                parameters: Vec::new(),
                variadic: false,
                calling_convention: CallingConvention::FarPascal,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks: vec![
                Block {
                    id: BlockId::new(0),
                    instructions: Vec::new(),
                    terminator: Terminator::Branch {
                        condition: Operand::Constant(TypedConstant {
                            type_id: TypeId::new(0),
                            value: Constant::Integer(1),
                        }),
                        then_block: BlockId::new(1),
                        else_block: BlockId::new(1),
                    },
                },
                Block {
                    id: BlockId::new(1),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(None),
                },
            ],
        };

        let cfg = ControlFlowGraph::analyze(&function).unwrap();

        assert_eq!(cfg.entry(), Some(BlockId::new(0)));
        assert_eq!(
            cfg.successors(BlockId::new(0)).unwrap(),
            &BTreeSet::from([BlockId::new(1)])
        );
        assert_eq!(
            cfg.predecessors(BlockId::new(1)).unwrap(),
            &BTreeSet::from([BlockId::new(0)])
        );
    }
}
