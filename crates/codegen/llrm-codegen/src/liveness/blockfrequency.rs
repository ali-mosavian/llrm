//! `MachineBlockFrequencyInfo` over `MachineBranchProbabilityInfo`, translated
//! from llvm/lib/CodeGen/MachineBlockFrequencyInfo.cpp and
//! MachineBranchProbabilityInfo.cpp at d1106deb71cc: a LIR body's blocks as
//! the graph of `llrm_mir::blockfrequency`'s one translation of
//! `BlockFrequencyInfoImpl`, its edges' probabilities isel's odds.
//!
//! A block is its index in `body.blocks`, the entry first; its successors are
//! `succ` without repeats, as a `MachineBasicBlock`'s are.
//!
//! DEVIATE 1: an edge isel gave no odds is LLVM's unknown probability; a block
//! none of whose edges have odds is one whose `Probs` is empty.

use llrm_mir::blockfrequency::{BlockFrequency, BlockFrequencyInfo, BranchProbability, EdgeProbabilities, FlowGraph};
use llrm_mir::blocklist::BlockList;
use llrm_mir::cycleinfo::CycleInfo;
use llrm_mir::domtree::DomGraph;
use llrm_mir::module::BlockId;

use crate::model::lir::LirBody;
use crate::support::hash::HashMap;

/// A LIR body as the graph its frequencies are worked out over.
pub(crate) struct Graph<'a> {
    body: &'a LirBody,
    blocks: Vec<BlockId>,
    succs: Vec<BlockList>,
    preds: Vec<BlockList>,
}

thread_local! {
    /// Graphs built, counted for the test that fixes one a round of the sink.
    #[cfg(test)]
    pub(crate) static GRAPH_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl<'a> Graph<'a> {
    pub(crate) fn of(body: &'a LirBody) -> Self {
        #[cfg(test)]
        GRAPH_BUILDS.with(|builds| builds.set(builds.get() + 1));
        let index: HashMap<i64, u32> =
            body.blocks.iter().enumerate().map(|(at, block)| (block.at, at as u32)).collect();
        let mut succs = vec![BlockList::new(); body.blocks.len()];
        let mut preds = vec![BlockList::new(); body.blocks.len()];
        for (from, block) in body.blocks.iter().enumerate() {
            for to in block.succ.iter() {
                let Some(&to) = index.get(to) else { continue };
                if !succs[from].contains(&BlockId(to)) {
                    succs[from].push(BlockId(to));
                    preds[to as usize].push(BlockId(from as u32));
                }
            }
        }
        Self { body, blocks: (0..body.blocks.len() as u32).map(BlockId).collect(), succs, preds }
    }
}

impl Graph<'_> {
    /// `MachineCycleInfo` of this graph.
    pub(crate) fn cycle_info(&self) -> CycleInfo {
        CycleInfo::compute(self)
    }
}

impl DomGraph for Graph<'_> {
    fn entry(&self) -> Option<BlockId> {
        (!self.blocks.is_empty()).then_some(BlockId(0))
    }

    fn max_number(&self) -> usize {
        self.blocks.len()
    }

    fn nodes(&self) -> &[BlockId] {
        &self.blocks
    }

    fn successors(
        &self,
        block: BlockId,
    ) -> BlockList {
        self.succs[block.0 as usize].clone()
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> BlockList {
        self.preds[block.0 as usize].clone()
    }
}

impl FlowGraph for Graph<'_> {}

impl EdgeProbabilities for Graph<'_> {
    /// `MachineBranchProbabilityInfo::getEdgeProbability(Src, SuccIdx)`: the
    /// block's `getSuccProbability`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBasicBlock.cpp#L1634>
    fn get_edge_probability(
        &self,
        src: BlockId,
        index: usize,
    ) -> BranchProbability {
        let from = &self.body.blocks[src.0 as usize];
        let succs = &self.succs[src.0 as usize];
        let probs: Vec<Option<u32>> = succs
            .iter()
            .map(|to| self.body.odds.taken.get(&(from.at, self.body.blocks[to.0 as usize].at)).copied())
            .collect();
        if probs.iter().all(Option::is_none) {
            return BranchProbability::new(1, succs.len() as u32);
        }
        if let Some(prob) = probs[index] {
            return BranchProbability::get_raw(prob);
        }
        let mut sum = BranchProbability::get_zero();
        let mut known = 0u32;
        for prob in probs.iter().flatten() {
            sum += BranchProbability::get_raw(*prob);
            known += 1;
        }
        sum.get_compl().div_u32(probs.len() as u32 - known)
    }
}

/// `MachineBranchProbabilityInfo` of one body: a block's edge probabilities by
/// its index in `body.blocks` and the position of the successor among the
/// block's distinct successors (`succ` without repeats, as a
/// `MachineBasicBlock`'s are). An edge isel gave no odds shares what the
/// block's stated edges leave.
pub struct MachineBranchProbabilityInfo<'a> {
    graph: std::rc::Rc<Graph<'a>>,
}

impl<'a> MachineBranchProbabilityInfo<'a> {
    pub fn of(body: &'a LirBody) -> Self {
        Self::of_with(std::rc::Rc::new(Graph::of(body)))
    }

    /// Over a graph already built: one build of a body feeds this, the
    /// block frequencies and a caller that walks it too.
    pub(crate) fn of_with(graph: std::rc::Rc<Graph<'a>>) -> Self {
        Self { graph }
    }

    /// `getEdgeProbability(Src, SuccIdx)`: numerator over 2^31.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBranchProbabilityInfo.cpp#L75>
    pub fn get_edge_probability(
        &self,
        block: usize,
        succ_index: usize,
    ) -> BranchProbability {
        self.graph.get_edge_probability(BlockId(block as u32), succ_index)
    }

    /// `isEdgeHot`: more than 4/5 (`StaticLikelyProb` is 80).
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBranchProbabilityInfo.cpp#L87>
    pub fn is_edge_hot(
        &self,
        block: usize,
        succ_index: usize,
    ) -> bool {
        self.get_edge_probability(block, succ_index) > BranchProbability::new(80, 100)
    }
}

/// `MachineBlockFrequencyInfo` of one body: each block's frequency by its
/// index in `body.blocks`.
pub struct MachineBlockFrequencyInfo {
    bfi: std::rc::Rc<BlockFrequencyInfo>,
}

thread_local! {
    /// The frequencies of the last body asked, with the two hashes of what
    /// they are worked out from (`fingerprint`). Passes that leave the blocks,
    /// edges and odds alone ask the same of one body again and again
    /// (`MachineBlockFrequencyInfo` is one analysis the pin's passes share).
    static LAST: std::cell::RefCell<Option<((u64, u64), std::rc::Rc<BlockFrequencyInfo>)>> =
        const { std::cell::RefCell::new(None) };
}

/// Two seeded hashes of everything the frequencies of `body` are worked out
/// from: its blocks, their successors and the odds of its edges.
fn fingerprint(body: &LirBody) -> (u64, u64) {
    use std::hash::{Hash, Hasher};
    let mut first = rustc_hash::FxHasher::default();
    let mut second = rustc_hash::FxHasher::default();
    0x9e37_79b9_7f4a_7c15_u64.hash(&mut second);
    for hasher in [&mut first, &mut second] {
        body.blocks.len().hash(hasher);
        for block in body.blocks.iter() {
            block.at.hash(hasher);
            block.succ.len().hash(hasher);
            for to in block.succ.iter() {
                to.hash(hasher);
            }
        }
        for (edge, odds) in body.odds.taken.iter() {
            edge.hash(hasher);
            odds.hash(hasher);
        }
    }
    (first.finish(), second.finish())
}

impl MachineBlockFrequencyInfo {
    /// `MachineBlockFrequencyInfo::calculate`, the cycles found afresh
    /// (`MachineCycleInfo`).
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockFrequencyInfo.cpp#L224>
    pub fn of(body: &LirBody) -> Self {
        Self::kept(body, || {
            let graph = Graph::of(body);
            BlockFrequencyInfo::calculate(&graph, &graph, &graph.cycle_info())
        })
    }

    /// The frequencies of `body`: those kept if its blocks, edges and odds are
    /// the last asked's, else `calculate`d and kept.
    fn kept(
        body: &LirBody,
        calculate: impl FnOnce() -> BlockFrequencyInfo,
    ) -> Self {
        let key = fingerprint(body);
        if let Some(found) =
            LAST.with(|last| last.borrow().as_ref().filter(|(was, _)| *was == key).map(|(_, bfi)| bfi.clone()))
        {
            return Self { bfi: found };
        }
        let bfi = std::rc::Rc::new(calculate());
        LAST.with(|last| *last.borrow_mut() = Some((key, bfi.clone())));
        Self { bfi }
    }

    /// Over a graph and its cycle info already built.
    pub(crate) fn of_with(
        graph: &Graph<'_>,
        ci: &CycleInfo,
    ) -> Self {
        Self::kept(graph.body, || BlockFrequencyInfo::calculate(graph, graph, ci))
    }

    /// `getBlockFreq` of the block at `index` in `body.blocks`.
    pub fn get_block_freq(
        &self,
        index: usize,
    ) -> BlockFrequency {
        self.bfi.get_block_freq(BlockId(index as u32))
    }

    pub fn get_entry_freq(&self) -> BlockFrequency {
        self.bfi.get_entry_freq()
    }

    /// `getBlockFreqRelativeToEntryBlock`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/MachineBlockFrequencyInfo.h#L71>
    pub fn get_block_freq_relative_to_entry_block(
        &self,
        index: usize,
    ) -> f64 {
        let entry = self.get_entry_freq().get_frequency();
        assert!(entry != 0, "getEntryFreq() should not return 0 here!");
        self.get_block_freq(index).get_frequency() as f64 / entry as f64
    }
}

#[cfg(test)]
#[path = "blockfrequency_tests.rs"]
mod tests;
