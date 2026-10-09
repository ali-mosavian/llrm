//! Natural loops, as LLVM's LoopInfo: the target of an edge that it
//! dominates heads a loop, whose blocks are those reaching that edge's
//! source without passing the header. Back edges into one header make one
//! loop.

use std::collections::{BTreeMap, BTreeSet};

use crate::dominators::DominatorTree;
use crate::module::{BlockId, Function, MetadataNode, MetadataOperand};

#[derive(Clone, Debug, PartialEq)]
pub struct Loop {
    pub header: BlockId,
    pub blocks: BTreeSet<BlockId>,
    pub latches: Vec<BlockId>,
    /// The innermost loop holding this one.
    pub parent: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoopInfo {
    pub loops: Vec<Loop>,
    /// Each block's innermost loop.
    innermost: BTreeMap<BlockId, usize>,
}

impl LoopInfo {
    pub fn new(
        function: &Function,
        tree: &DominatorTree,
    ) -> Self {
        let mut latches: BTreeMap<BlockId, Vec<BlockId>> = BTreeMap::new();
        for &block in function.layout() {
            if !tree.is_reachable(block) {
                continue;
            }
            for successor in function.successors(block) {
                if tree.dominates(successor, block) && !latches.get(&successor).is_some_and(|one| one.contains(&block))
                {
                    latches.entry(successor).or_default().push(block);
                }
            }
        }
        let mut loops: Vec<Loop> = latches
            .into_iter()
            .map(|(header, latches)| {
                let mut blocks = BTreeSet::from([header]);
                let mut work = latches.clone();
                while let Some(block) = work.pop() {
                    if blocks.insert(block) {
                        work.extend(function.predecessors(block).into_iter().filter(|one| tree.is_reachable(*one)));
                    }
                }
                Loop { header, blocks, latches, parent: None }
            })
            .collect();
        // Outer loops first, so each block's innermost loop is its last.
        loops.sort_by_key(|one| std::cmp::Reverse(one.blocks.len()));
        let mut innermost = BTreeMap::new();
        for at in 0..loops.len() {
            loops[at].parent = (0..at).rev().find(|&outer| loops[outer].blocks.is_superset(&loops[at].blocks));
            for &block in &loops[at].blocks {
                innermost.insert(block, at);
            }
        }
        Self { loops, innermost }
    }

    /// The innermost loop holding `block`.
    pub fn loop_of(
        &self,
        block: BlockId,
    ) -> Option<&Loop> {
        self.innermost.get(&block).map(|&at| &self.loops[at])
    }

    /// Whether going from `from` to `to` leaves a loop: one holds `from`
    /// and not `to`.
    pub fn leaves(
        &self,
        from: BlockId,
        to: BlockId,
    ) -> bool {
        let mut at = self.innermost.get(&from).copied();
        while let Some(one) = at {
            if !self.loops[one].blocks.contains(&to) {
                return true;
            }
            at = self.loops[one].parent;
        }
        false
    }

    /// How many loops hold `block`: 0 outside any.
    pub fn depth(
        &self,
        block: BlockId,
    ) -> u32 {
        let mut depth = 0;
        let mut at = self.innermost.get(&block).copied();
        while let Some(one) = at {
            depth += 1;
            at = self.loops[one].parent;
        }
        depth
    }
}

/// Whether `one` has an edge out: a loop with none never ends, whatever marks it.
pub fn exits(
    function: &Function,
    one: &Loop,
) -> bool {
    !one.latches.is_empty()
        && one.blocks.iter().any(|&block| function.successors(block).iter().any(|next| !one.blocks.contains(next)))
}

/// Whether the loop `latch` closes carries `llvm.loop.mustprogress`.
pub fn marked(
    metadata: &[MetadataNode],
    function: &Function,
    latch: BlockId,
) -> bool {
    let Some(branch) = function.terminator(latch) else { return false };
    let Some((_, node)) = function.instruction(branch).metadata.iter().find(|(kind, _)| kind == "llvm.loop") else {
        return false;
    };
    let Some(node) = metadata.get(node.0 as usize) else { return false };
    node.operands
        .iter()
        .any(
            |one| match one {
                MetadataOperand::Node(id) => metadata.get(id.0 as usize).is_some_and(|property| {
                    matches!(
                        property.operands.first(),
                        Some(MetadataOperand::String(name)) if name == "llvm.loop.mustprogress"
                    )
                }),
                _ => false,
            },
        )
}

/// Whether the language promises every loop of `function` ends: each has an
/// edge out, and the function says so of every loop (`whole`) or each loop
/// is marked on all its latches (`llvm.loop.mustprogress`, as C11 6.8.5p6
/// gives only the loops whose controlling expression is not constant:
/// `for (;;)` hangs).
pub fn ends_by_promise(
    metadata: &[MetadataNode],
    function: &Function,
    whole: bool,
) -> bool {
    let tree = DominatorTree::new(function);
    LoopInfo::new(function, &tree)
        .loops
        .iter()
        .all(
            |one| exits(function, one) && (whole || one.latches.iter().all(|&latch| marked(metadata, function, latch))),
        )
}
