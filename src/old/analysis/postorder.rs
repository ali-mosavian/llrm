//! Deterministic reachable postorder traversal for one IR control-flow graph.

use std::collections::{BTreeMap, BTreeSet};

use super::{CfgError, ControlFlowGraph};
use crate::old::ir::{BlockId, Function};

/// A depth-first postorder rooted at a function's CFG entry block.
///
/// The traversal contains every reachable block exactly once. Successors are
/// visited in ascending block-ID order, matching [`ControlFlowGraph`]'s
/// deterministic successor ordering. Blocks not reachable from the entry are
/// deliberately absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostOrder {
    blocks: Vec<BlockId>,
    ranks: BTreeMap<BlockId, usize>,
}

impl PostOrder {
    /// Builds a reachable postorder for `function`.
    ///
    /// Malformed control flow is reported by [`CfgError`] while constructing
    /// the CFG. In particular, duplicate block IDs and terminators targeting
    /// absent blocks are rejected.
    pub fn analyze(function: &Function) -> Result<Self, CfgError> {
        let cfg = ControlFlowGraph::analyze(function)?;
        Ok(Self::from_cfg(&cfg))
    }

    /// Builds a reachable postorder from an already validated CFG.
    pub fn from_cfg(cfg: &ControlFlowGraph) -> Self {
        let Some(entry) = cfg.entry() else {
            return Self {
                blocks: Vec::new(),
                ranks: BTreeMap::new(),
            };
        };

        let mut blocks = Vec::new();
        let mut visited = BTreeSet::new();
        let mut pending = vec![(entry, false)];

        while let Some((block, expanded)) = pending.pop() {
            if expanded {
                blocks.push(block);
                continue;
            }
            if !visited.insert(block) {
                continue;
            }

            pending.push((block, true));
            pending.extend(
                cfg.successors(block)
                    .expect("reachable blocks are CFG blocks")
                    .iter()
                    .rev()
                    .copied()
                    .map(|successor| (successor, false)),
            );
        }

        let ranks = blocks
            .iter()
            .copied()
            .enumerate()
            .map(|(rank, block)| (block, rank))
            .collect();

        Self { blocks, ranks }
    }

    /// Returns reachable blocks in deterministic depth-first postorder.
    pub fn postorder(&self) -> &[BlockId] {
        &self.blocks
    }

    /// Iterates reachable blocks in deterministic reverse postorder.
    pub fn reverse_postorder(&self) -> impl DoubleEndedIterator<Item = BlockId> + '_ {
        self.blocks.iter().rev().copied()
    }

    /// Returns `block`'s zero-based rank in [`Self::postorder`].
    ///
    /// Unreachable and absent blocks have no rank.
    pub fn rank(&self, block: BlockId) -> Option<usize> {
        self.ranks.get(&block).copied()
    }

    /// Returns whether `block` is reachable from the CFG entry.
    pub fn is_reachable(&self, block: BlockId) -> bool {
        self.ranks.contains_key(&block)
    }
}

#[cfg(test)]
mod tests {
    use super::PostOrder;
    use crate::old::ir::{
        Block, BlockId, CallingConvention, Constant, Function, FunctionId, Linkage, Operand,
        Signature, Terminator, TypeId, TypedConstant,
    };

    #[test]
    fn orders_a_diamond_deterministically() {
        let postorder = PostOrder::analyze(&function(vec![
            block(
                0,
                Terminator::Branch {
                    condition: condition(),
                    then_block: BlockId::new(1),
                    else_block: BlockId::new(2),
                },
            ),
            block(1, Terminator::Jump(BlockId::new(3))),
            block(2, Terminator::Jump(BlockId::new(3))),
            block(3, Terminator::Return(None)),
        ]))
        .unwrap();

        assert_eq!(
            postorder.postorder(),
            [
                BlockId::new(3),
                BlockId::new(1),
                BlockId::new(2),
                BlockId::new(0),
            ]
        );
        assert_eq!(
            postorder.reverse_postorder().collect::<Vec<_>>(),
            [
                BlockId::new(0),
                BlockId::new(2),
                BlockId::new(1),
                BlockId::new(3),
            ]
        );
        assert_eq!(postorder.rank(BlockId::new(3)), Some(0));
        assert_eq!(postorder.rank(BlockId::new(0)), Some(3));
    }

    #[test]
    fn orders_a_loop_once() {
        let postorder = PostOrder::analyze(&function(vec![
            block(0, Terminator::Jump(BlockId::new(1))),
            block(
                1,
                Terminator::Branch {
                    condition: condition(),
                    then_block: BlockId::new(2),
                    else_block: BlockId::new(3),
                },
            ),
            block(2, Terminator::Jump(BlockId::new(1))),
            block(3, Terminator::Return(None)),
        ]))
        .unwrap();

        assert_eq!(
            postorder.postorder(),
            [
                BlockId::new(2),
                BlockId::new(3),
                BlockId::new(1),
                BlockId::new(0),
            ]
        );
        assert_eq!(postorder.rank(BlockId::new(1)), Some(2));
    }

    #[test]
    fn excludes_unreachable_blocks() {
        let postorder = PostOrder::analyze(&function(vec![
            block(0, Terminator::Jump(BlockId::new(1))),
            block(1, Terminator::Return(None)),
            block(2, Terminator::Jump(BlockId::new(2))),
        ]))
        .unwrap();

        assert_eq!(postorder.postorder(), [BlockId::new(1), BlockId::new(0)]);
        assert!(!postorder.is_reachable(BlockId::new(2)));
        assert_eq!(postorder.rank(BlockId::new(2)), None);
    }

    #[test]
    fn declarations_have_no_traversal() {
        let postorder = PostOrder::analyze(&function(Vec::new())).unwrap();

        assert!(postorder.postorder().is_empty());
        assert!(postorder.reverse_postorder().next().is_none());
        assert_eq!(postorder.rank(BlockId::new(0)), None);
    }

    fn function(blocks: Vec<Block>) -> Function {
        Function {
            id: FunctionId::new(0),
            name: "postorder".into(),
            signature: Signature {
                result: TypeId::new(0),
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

    fn block(id: u32, terminator: Terminator) -> Block {
        Block {
            id: BlockId::new(id),
            instructions: Vec::new(),
            terminator,
        }
    }

    fn condition() -> Operand {
        Operand::Constant(TypedConstant {
            type_id: TypeId::new(0),
            value: Constant::Integer(1),
        })
    }
}
