//! Natural loops, as LLVM's LoopInfo: the target of an edge that it
//! dominates heads a loop, whose blocks are those reaching that edge's
//! source without passing the header. Back edges into one header make one
//! loop.

use std::collections::{BTreeMap, BTreeSet};

use crate::dominators::DominatorTree;
use crate::module::{BlockId, Function};

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
    pub fn new(function: &Function, tree: &DominatorTree) -> Self {
        let mut latches: BTreeMap<BlockId, Vec<BlockId>> = BTreeMap::new();
        for &block in function.layout() {
            if !tree.is_reachable(block) {
                continue;
            }
            for successor in function.successors(block) {
                if tree.dominates(successor, block) && !latches.get(&successor).is_some_and(|one| one.contains(&block)) {
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
    pub fn loop_of(&self, block: BlockId) -> Option<&Loop> {
        self.innermost.get(&block).map(|&at| &self.loops[at])
    }

    /// Whether going from `from` to `to` leaves a loop: one holds `from`
    /// and not `to`.
    pub fn leaves(&self, from: BlockId, to: BlockId) -> bool {
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
    pub fn depth(&self, block: BlockId) -> u32 {
        let mut depth = 0;
        let mut at = self.innermost.get(&block).copied();
        while let Some(one) = at {
            depth += 1;
            at = self.loops[one].parent;
        }
        depth
    }
}
