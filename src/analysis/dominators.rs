//! Deterministic dominator information for one IR control-flow graph.

use std::collections::{BTreeMap, BTreeSet};

use super::{CfgError, ControlFlowGraph};
use crate::ir::{BlockId, Function};

/// Dominator information rooted at a function's CFG entry block.
///
/// Only blocks reachable from [`Self::entry`] are part of this analysis. An
/// unreachable block has no immediate dominator, no dominator-tree membership,
/// and does not dominate itself. Reachable blocks deliberately do dominate
/// themselves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dominators {
    entry: Option<BlockId>,
    reachable: BTreeSet<BlockId>,
    immediate_dominators: BTreeMap<BlockId, BlockId>,
    children: BTreeMap<BlockId, BTreeSet<BlockId>>,
}

impl Dominators {
    /// Builds dominator information for `function`.
    ///
    /// Malformed control flow is reported by [`CfgError`] while constructing
    /// the CFG. In particular, duplicate block IDs and terminators targeting
    /// absent blocks are rejected.
    pub fn analyze(function: &Function) -> Result<Self, CfgError> {
        let cfg = ControlFlowGraph::analyze(function)?;
        Ok(Self::from_cfg(&cfg))
    }

    /// Builds dominator information from an already validated CFG.
    pub fn from_cfg(cfg: &ControlFlowGraph) -> Self {
        let Some(entry) = cfg.entry() else {
            return Self {
                entry: None,
                reachable: BTreeSet::new(),
                immediate_dominators: BTreeMap::new(),
                children: BTreeMap::new(),
            };
        };

        let reachable = reachable_blocks(cfg, entry);
        let sets = dominator_sets(cfg, entry, &reachable);
        let mut immediate_dominators = BTreeMap::new();
        let mut children = reachable
            .iter()
            .copied()
            .map(|block| (block, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();

        for block in reachable.iter().copied().filter(|block| *block != entry) {
            let dominators = sets
                .get(&block)
                .expect("every reachable block has a dominator set");
            let strict_dominators = dominators
                .iter()
                .copied()
                .filter(|dominator| *dominator != block)
                .collect::<BTreeSet<_>>();
            let immediate_dominator = strict_dominators
                .iter()
                .copied()
                .find(|candidate| {
                    strict_dominators.iter().all(|other| {
                        other == candidate
                            || sets
                                .get(candidate)
                                .expect("every strict dominator is reachable")
                                .contains(other)
                    })
                })
                .expect("every reachable non-entry block has an immediate dominator");

            immediate_dominators.insert(block, immediate_dominator);
            children
                .get_mut(&immediate_dominator)
                .expect("immediate dominators are reachable")
                .insert(block);
        }

        Self {
            entry: Some(entry),
            reachable,
            immediate_dominators,
            children,
        }
    }

    /// Returns the CFG entry, or `None` for a declaration or empty function.
    pub const fn entry(&self) -> Option<BlockId> {
        self.entry
    }

    /// Returns whether `block` is reachable from the entry.
    pub fn is_reachable(&self, block: BlockId) -> bool {
        self.reachable.contains(&block)
    }

    /// Iterates over reachable blocks in ascending block-ID order.
    pub fn reachable_blocks(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.reachable.iter().copied()
    }

    /// Returns `block`'s immediate dominator.
    ///
    /// The entry and unreachable blocks have no immediate dominator.
    pub fn immediate_dominator(&self, block: BlockId) -> Option<BlockId> {
        self.immediate_dominators.get(&block).copied()
    }

    /// Iterates from `block` through its immediate dominators to the entry.
    ///
    /// The returned chain includes `block` itself. It is `None` for an
    /// unreachable or absent block.
    pub fn dominator_chain(&self, block: BlockId) -> Option<impl Iterator<Item = BlockId> + '_> {
        self.is_reachable(block).then(|| {
            std::iter::successors(Some(block), move |current| {
                self.immediate_dominator(*current)
            })
        })
    }

    /// Returns the blocks immediately dominated by `block`, in block-ID order.
    ///
    /// A reachable leaf returns an empty set. Unreachable and absent blocks
    /// return `None`, because they are not members of the dominator tree.
    pub fn children(&self, block: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.children.get(&block)
    }

    /// Returns whether every entry-to-`block` path contains `dominator`.
    ///
    /// Both blocks must be reachable. Therefore this returns `false` for an
    /// unreachable block even when `dominator == block`; a reachable block
    /// dominates itself.
    pub fn dominates(&self, dominator: BlockId, block: BlockId) -> bool {
        self.dominator_chain(block)
            .is_some_and(|mut chain| chain.any(|candidate| candidate == dominator))
    }
}

fn reachable_blocks(cfg: &ControlFlowGraph, entry: BlockId) -> BTreeSet<BlockId> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![entry];

    while let Some(block) = pending.pop() {
        if !reachable.insert(block) {
            continue;
        }
        pending.extend(
            cfg.successors(block)
                .expect("reachable blocks are CFG blocks")
                .iter()
                .copied(),
        );
    }

    reachable
}

fn dominator_sets(
    cfg: &ControlFlowGraph,
    entry: BlockId,
    reachable: &BTreeSet<BlockId>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let mut sets = reachable
        .iter()
        .copied()
        .map(|block| {
            let initial = if block == entry {
                BTreeSet::from([entry])
            } else {
                reachable.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();

    let mut changed = true;
    while changed {
        changed = false;
        for block in reachable.iter().copied().filter(|block| *block != entry) {
            let predecessors = cfg
                .predecessors(block)
                .expect("reachable blocks are CFG blocks");
            let mut predecessors = predecessors
                .iter()
                .copied()
                .filter(|predecessor| reachable.contains(predecessor));
            let first = predecessors
                .next()
                .expect("reachable non-entry blocks have a reachable predecessor");
            let mut next = sets
                .get(&first)
                .expect("reachable predecessors have dominator sets")
                .clone();

            for predecessor in predecessors {
                next = next
                    .intersection(
                        sets.get(&predecessor)
                            .expect("reachable predecessors have dominator sets"),
                    )
                    .copied()
                    .collect();
            }
            next.insert(block);

            if sets.get(&block) != Some(&next) {
                sets.insert(block, next);
                changed = true;
            }
        }
    }

    sets
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::Dominators;
    use crate::ir::{
        Block, BlockId, CallingConvention, Constant, Function, FunctionId, Linkage, Operand,
        Signature, Terminator, TypeId, TypedConstant,
    };

    #[test]
    fn builds_exact_dominators_for_a_diamond() {
        let dominators = Dominators::analyze(&function(vec![
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

        assert_eq!(dominators.entry(), Some(BlockId::new(0)));
        assert_eq!(
            dominators.reachable_blocks().collect::<Vec<_>>(),
            vec![
                BlockId::new(0),
                BlockId::new(1),
                BlockId::new(2),
                BlockId::new(3),
            ]
        );
        assert_eq!(dominators.immediate_dominator(BlockId::new(0)), None);
        assert_eq!(
            dominators.immediate_dominator(BlockId::new(1)),
            Some(BlockId::new(0))
        );
        assert_eq!(
            dominators.immediate_dominator(BlockId::new(2)),
            Some(BlockId::new(0))
        );
        assert_eq!(
            dominators.immediate_dominator(BlockId::new(3)),
            Some(BlockId::new(0))
        );
        assert_eq!(
            dominators.children(BlockId::new(0)),
            Some(&BTreeSet::from([
                BlockId::new(1),
                BlockId::new(2),
                BlockId::new(3),
            ]))
        );
        assert_eq!(
            dominators
                .dominator_chain(BlockId::new(3))
                .unwrap()
                .collect::<Vec<_>>(),
            vec![BlockId::new(3), BlockId::new(0)]
        );
        assert!(dominators.dominates(BlockId::new(0), BlockId::new(3)));
        assert!(dominators.dominates(BlockId::new(3), BlockId::new(3)));
        assert!(!dominators.dominates(BlockId::new(1), BlockId::new(3)));
    }

    #[test]
    fn handles_a_backedge_loop() {
        let dominators = Dominators::analyze(&function(vec![
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
            dominators.immediate_dominator(BlockId::new(1)),
            Some(BlockId::new(0))
        );
        assert_eq!(
            dominators.immediate_dominator(BlockId::new(2)),
            Some(BlockId::new(1))
        );
        assert_eq!(
            dominators.immediate_dominator(BlockId::new(3)),
            Some(BlockId::new(1))
        );
        assert!(dominators.dominates(BlockId::new(1), BlockId::new(2)));
        assert!(dominators.dominates(BlockId::new(1), BlockId::new(3)));
        assert!(!dominators.dominates(BlockId::new(2), BlockId::new(1)));
    }

    #[test]
    fn excludes_unreachable_blocks_from_the_dominator_tree() {
        let dominators = Dominators::analyze(&function(vec![
            block(0, Terminator::Jump(BlockId::new(1))),
            block(1, Terminator::Return(None)),
            block(2, Terminator::Jump(BlockId::new(2))),
        ]))
        .unwrap();

        assert!(!dominators.is_reachable(BlockId::new(2)));
        assert_eq!(dominators.immediate_dominator(BlockId::new(2)), None);
        assert_eq!(dominators.children(BlockId::new(2)), None);
        assert!(dominators.dominator_chain(BlockId::new(2)).is_none());
        assert!(!dominators.dominates(BlockId::new(2), BlockId::new(2)));
        assert!(!dominators.dominates(BlockId::new(0), BlockId::new(2)));
    }

    #[test]
    fn handles_empty_and_single_block_functions() {
        let empty = Dominators::analyze(&function(Vec::new())).unwrap();
        assert_eq!(empty.entry(), None);
        assert!(empty.reachable_blocks().next().is_none());
        assert!(!empty.dominates(BlockId::new(0), BlockId::new(0)));

        let single =
            Dominators::analyze(&function(vec![block(4, Terminator::Return(None))])).unwrap();
        assert_eq!(single.entry(), Some(BlockId::new(4)));
        assert!(single.is_reachable(BlockId::new(4)));
        assert_eq!(single.immediate_dominator(BlockId::new(4)), None);
        assert!(
            single
                .children(BlockId::new(4))
                .is_some_and(BTreeSet::is_empty)
        );
        assert_eq!(
            single
                .dominator_chain(BlockId::new(4))
                .unwrap()
                .collect::<Vec<_>>(),
            vec![BlockId::new(4)]
        );
        assert!(single.dominates(BlockId::new(4), BlockId::new(4)));
    }

    fn function(blocks: Vec<Block>) -> Function {
        Function {
            id: FunctionId::new(0),
            name: "dominators".into(),
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
