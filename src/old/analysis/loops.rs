//! Deterministic natural-loop information for one IR control-flow graph.

use std::collections::{BTreeMap, BTreeSet};

use super::{CfgError, ControlFlowGraph, Dominators};
use crate::old::ir::{BlockId, Function};

/// Natural-loop information for a function.
///
/// The analysis considers only reachable backedges `latch -> header` for
/// which `header` dominates `latch`. It therefore excludes unreachable cycles
/// and irreducible cycles with no dominating header. An irreducible region can
/// still contain a separately natural, dominated loop, which is reported in
/// the ordinary way.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopInfo {
    loops: BTreeMap<BlockId, NaturalLoop>,
}

/// One natural loop, identified by its header block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NaturalLoop {
    header: BlockId,
    latches: BTreeSet<BlockId>,
    blocks: BTreeSet<BlockId>,
    exiting_blocks: BTreeSet<BlockId>,
    exit_blocks: BTreeSet<BlockId>,
    parent: Option<BlockId>,
    children: BTreeSet<BlockId>,
}

impl LoopInfo {
    /// Builds natural-loop information for `function`.
    ///
    /// Malformed control flow is rejected by [`CfgError`] while constructing
    /// the CFG. The analysis does not modify the function or its CFG.
    pub fn analyze(function: &Function) -> Result<Self, CfgError> {
        let cfg = ControlFlowGraph::analyze(function)?;
        let dominators = Dominators::from_cfg(&cfg);
        let reachable = dominators.reachable_blocks().collect::<BTreeSet<_>>();
        let mut latches_by_header = BTreeMap::<BlockId, BTreeSet<BlockId>>::new();

        for latch in reachable.iter().copied() {
            for header in cfg
                .successors(latch)
                .expect("reachable blocks are CFG blocks")
            {
                if dominators.dominates(*header, latch) {
                    latches_by_header.entry(*header).or_default().insert(latch);
                }
            }
        }

        let mut loops = latches_by_header
            .into_iter()
            .map(|(header, latches)| {
                let blocks = natural_loop_blocks(&cfg, &reachable, header, &latches);
                let (exiting_blocks, exit_blocks) = exit_blocks(&cfg, &blocks);
                (
                    header,
                    NaturalLoop {
                        header,
                        latches,
                        blocks,
                        exiting_blocks,
                        exit_blocks,
                        parent: None,
                        children: BTreeSet::new(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        let parents = loops
            .iter()
            .map(|(header, loop_)| {
                let parent = loops
                    .iter()
                    .filter(|(candidate, candidate_loop)| {
                        **candidate != *header
                            && candidate_loop.blocks.len() > loop_.blocks.len()
                            && candidate_loop.blocks.is_superset(&loop_.blocks)
                    })
                    .min_by_key(|(candidate, candidate_loop)| {
                        (candidate_loop.blocks.len(), **candidate)
                    })
                    .map(|(candidate, _)| *candidate);
                (*header, parent)
            })
            .collect::<BTreeMap<_, _>>();

        for (header, parent) in parents {
            loops
                .get_mut(&header)
                .expect("parent calculation retains every loop")
                .parent = parent;
            if let Some(parent) = parent {
                loops
                    .get_mut(&parent)
                    .expect("parents are natural-loop headers")
                    .children
                    .insert(header);
            }
        }

        Ok(Self { loops })
    }

    /// Iterates over loops in ascending header block-ID order.
    pub fn loops(&self) -> impl Iterator<Item = &NaturalLoop> {
        self.loops.values()
    }

    /// Returns the natural loop headed by `header`, if it has one.
    pub fn loop_for_header(&self, header: BlockId) -> Option<&NaturalLoop> {
        self.loops.get(&header)
    }

    /// Returns the most deeply nested loop containing `block`.
    ///
    /// Loop nesting uses strict block-set containment. If malformed-but-valid
    /// control flow produces equally small overlapping candidates, the lower
    /// header block ID is chosen to keep the answer deterministic.
    pub fn innermost_loop(&self, block: BlockId) -> Option<&NaturalLoop> {
        self.loops
            .values()
            .filter(|loop_| loop_.contains(block))
            .min_by_key(|loop_| (loop_.blocks.len(), loop_.header))
    }
}

impl NaturalLoop {
    /// Returns this loop's header.
    pub const fn header(&self) -> BlockId {
        self.header
    }

    /// Returns all backedge source blocks for this header.
    pub const fn latches(&self) -> &BTreeSet<BlockId> {
        &self.latches
    }

    /// Returns every block in the loop.
    pub const fn blocks(&self) -> &BTreeSet<BlockId> {
        &self.blocks
    }

    /// Returns whether `block` belongs to this loop.
    pub fn contains(&self, block: BlockId) -> bool {
        self.blocks.contains(&block)
    }

    /// Returns loop blocks that have an edge to outside the loop.
    pub const fn exiting_blocks(&self) -> &BTreeSet<BlockId> {
        &self.exiting_blocks
    }

    /// Returns blocks outside the loop reached by an exiting edge.
    pub const fn exit_blocks(&self) -> &BTreeSet<BlockId> {
        &self.exit_blocks
    }

    /// Returns the header of this loop's immediate containing loop.
    pub const fn parent(&self) -> Option<BlockId> {
        self.parent
    }

    /// Returns headers of loops immediately nested in this loop.
    pub const fn children(&self) -> &BTreeSet<BlockId> {
        &self.children
    }
}

fn natural_loop_blocks(
    cfg: &ControlFlowGraph,
    reachable: &BTreeSet<BlockId>,
    header: BlockId,
    latches: &BTreeSet<BlockId>,
) -> BTreeSet<BlockId> {
    let mut blocks = BTreeSet::from([header]);
    let mut pending = latches.clone();

    while let Some(block) = pending.pop_first() {
        if !blocks.insert(block) {
            continue;
        }
        for predecessor in cfg.predecessors(block).expect("loop blocks are CFG blocks") {
            if reachable.contains(predecessor) && *predecessor != header {
                pending.insert(*predecessor);
            }
        }
    }

    blocks
}

fn exit_blocks(
    cfg: &ControlFlowGraph,
    blocks: &BTreeSet<BlockId>,
) -> (BTreeSet<BlockId>, BTreeSet<BlockId>) {
    let mut exiting_blocks = BTreeSet::new();
    let mut exit_blocks = BTreeSet::new();

    for block in blocks {
        for successor in cfg.successors(*block).expect("loop blocks are CFG blocks") {
            if !blocks.contains(successor) {
                exiting_blocks.insert(*block);
                exit_blocks.insert(*successor);
            }
        }
    }

    (exiting_blocks, exit_blocks)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::LoopInfo;
    use crate::old::ir::{
        Block, BlockId, CallingConvention, Constant, Function, FunctionId, Linkage, Operand,
        Signature, Terminator, TypeId, TypedConstant,
    };

    #[test]
    fn records_exact_membership_latches_and_exits() {
        let loops = LoopInfo::analyze(&function(vec![
            block(0, Terminator::Jump(BlockId::new(1))),
            block(
                1,
                Terminator::Branch {
                    condition: condition(),
                    then_block: BlockId::new(2),
                    else_block: BlockId::new(4),
                },
            ),
            block(
                2,
                Terminator::Branch {
                    condition: condition(),
                    then_block: BlockId::new(1),
                    else_block: BlockId::new(3),
                },
            ),
            block(3, Terminator::Jump(BlockId::new(1))),
            block(4, Terminator::Return(None)),
        ]))
        .unwrap();

        let loop_ = loops.loop_for_header(BlockId::new(1)).unwrap();
        assert_eq!(
            loops
                .loops()
                .map(|loop_| loop_.header())
                .collect::<Vec<_>>(),
            [BlockId::new(1)]
        );
        assert_eq!(
            loop_.blocks(),
            &BTreeSet::from([BlockId::new(1), BlockId::new(2), BlockId::new(3)])
        );
        assert_eq!(
            loop_.latches(),
            &BTreeSet::from([BlockId::new(2), BlockId::new(3)])
        );
        assert_eq!(loop_.exiting_blocks(), &BTreeSet::from([BlockId::new(1)]));
        assert_eq!(loop_.exit_blocks(), &BTreeSet::from([BlockId::new(4)]));
    }

    #[test]
    fn relates_nested_loops_and_finds_the_innermost_loop() {
        let loops = LoopInfo::analyze(&function(vec![
            block(0, Terminator::Jump(BlockId::new(1))),
            block(
                1,
                Terminator::Branch {
                    condition: condition(),
                    then_block: BlockId::new(2),
                    else_block: BlockId::new(6),
                },
            ),
            block(
                2,
                Terminator::Branch {
                    condition: condition(),
                    then_block: BlockId::new(3),
                    else_block: BlockId::new(5),
                },
            ),
            block(3, Terminator::Jump(BlockId::new(4))),
            block(4, Terminator::Jump(BlockId::new(2))),
            block(5, Terminator::Jump(BlockId::new(1))),
            block(6, Terminator::Return(None)),
        ]))
        .unwrap();

        let outer = loops.loop_for_header(BlockId::new(1)).unwrap();
        let inner = loops.loop_for_header(BlockId::new(2)).unwrap();
        assert_eq!(outer.parent(), None);
        assert_eq!(outer.children(), &BTreeSet::from([BlockId::new(2)]));
        assert_eq!(inner.parent(), Some(BlockId::new(1)));
        assert!(inner.children().is_empty());
        assert_eq!(
            loops
                .innermost_loop(BlockId::new(3))
                .map(|loop_| loop_.header()),
            Some(BlockId::new(2))
        );
        assert_eq!(
            loops
                .innermost_loop(BlockId::new(1))
                .map(|loop_| loop_.header()),
            Some(BlockId::new(1))
        );
        assert_eq!(loops.innermost_loop(BlockId::new(6)), None);
    }

    #[test]
    fn excludes_irreducible_and_unreachable_cycles() {
        let loops = LoopInfo::analyze(&function(vec![
            block(
                0,
                Terminator::Branch {
                    condition: condition(),
                    then_block: BlockId::new(1),
                    else_block: BlockId::new(2),
                },
            ),
            block(1, Terminator::Jump(BlockId::new(2))),
            block(
                2,
                Terminator::Branch {
                    condition: condition(),
                    then_block: BlockId::new(1),
                    else_block: BlockId::new(3),
                },
            ),
            block(3, Terminator::Return(None)),
            block(4, Terminator::Jump(BlockId::new(4))),
        ]))
        .unwrap();

        assert!(loops.loops().next().is_none());
        assert_eq!(loops.innermost_loop(BlockId::new(4)), None);
    }

    #[test]
    fn handles_declarations_and_functions_without_loops() {
        let declaration = LoopInfo::analyze(&function(Vec::new())).unwrap();
        assert!(declaration.loops().next().is_none());

        let straight_line = LoopInfo::analyze(&function(vec![
            block(0, Terminator::Jump(BlockId::new(1))),
            block(1, Terminator::Return(None)),
        ]))
        .unwrap();
        assert!(straight_line.loops().next().is_none());
    }

    fn function(blocks: Vec<Block>) -> Function {
        Function {
            id: FunctionId::new(0),
            name: "loops".into(),
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
