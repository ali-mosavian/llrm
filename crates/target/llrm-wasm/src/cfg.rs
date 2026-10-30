//! The control-flow graph of a wasm [`Function`], for the dominator tree and loop info of
//! `llrm-mir`, which are written over a graph trait.
//!
//! Blocks are numbered by position: `MachineBasicBlock::getNumber` after `RenumberBlocks`.

use llrm_mir::domtree::{DomGraph, DomTree};
use llrm_mir::loopinfo::{LoopGraph, LoopInfo};
use llrm_mir::module::BlockId;

use crate::mir::Function;

/// Successors and predecessors by block number, each block once per edge.
pub struct Cfg {
    nodes: Vec<BlockId>,
    succs: Vec<Vec<BlockId>>,
    preds: Vec<Vec<BlockId>>,
}

impl Cfg {
    pub fn of(f: &Function) -> Cfg {
        let n = f.blocks.len();
        let succs: Vec<Vec<BlockId>> =
            f.succs.iter().map(|to| to.iter().map(|&b| BlockId(b as u32)).collect()).collect();
        let mut preds = vec![Vec::new(); n];
        for (from, to) in succs.iter().enumerate() {
            for &block in to {
                preds[block.0 as usize].push(BlockId(from as u32));
            }
        }
        Cfg { nodes: (0..n as u32).map(BlockId).collect(), succs, preds }
    }

    pub fn predecessors_of(
        &self,
        block: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        self.preds[block].iter().map(|b| b.0 as usize)
    }

    /// The dominator tree and the loops.
    pub fn analyses(&self) -> (DomTree, LoopInfo<BlockId>) {
        let mut tree = DomTree::new();
        tree.recalculate(self);
        let loops = LoopInfo::analyze(self, || tree.clone());
        (tree, loops)
    }
}

impl DomGraph for Cfg {
    fn entry(&self) -> Option<BlockId> {
        self.nodes.first().copied()
    }

    fn max_number(&self) -> usize {
        self.nodes.len()
    }

    fn nodes(&self) -> &[BlockId] {
        &self.nodes
    }

    fn successors(
        &self,
        block: BlockId,
    ) -> llrm_mir::blocklist::BlockList {
        self.succs[block.0 as usize].as_slice().into()
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> llrm_mir::blocklist::BlockList {
        self.preds[block.0 as usize].as_slice().into()
    }
}

impl LoopGraph for Cfg {
    type Block = BlockId;

    fn entry(&self) -> Option<BlockId> {
        self.nodes.first().copied()
    }

    fn max_number(&self) -> usize {
        self.nodes.len()
    }

    fn child(
        &self,
        block: BlockId,
        cursor: &mut usize,
    ) -> Option<BlockId> {
        let next = self.succs[block.0 as usize].get(*cursor).copied();
        *cursor += 1;
        next
    }

    fn inverse_children(
        &self,
        block: BlockId,
    ) -> impl Iterator<Item = BlockId> {
        self.preds[block.0 as usize].iter().copied()
    }
}
