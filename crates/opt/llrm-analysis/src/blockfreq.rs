//! `BlockFrequencyInfo` of a MIR function: LLVM's `BlockFrequencyInfoImpl`
//! (`llrm_mir::blockfrequency`) over `branchprob`'s edge probabilities, as
//! `BlockFrequencyInfoWrapperPass` runs it over `BranchProbabilityInfo`.
//!
//! DEVIATE D1b: the edge probabilities are `branchprob::estimated`'s
//! heuristics (LLVM's loop, call and return weights, and GCC's), not LLVM's
//! `BranchProbabilityInfo`, and no trip count stands in for the loop
//! heuristic, as LLVM's has none. Exit: `BranchProbabilityInfo` translated.
//! With `branch_weights` metadata on every branch the two agree.
//!
//! # Used by
//! `inlinecost` (cold and hot call sites, D1).

use llrm_mir::blockfrequency::{BlockFrequency, BlockFrequencyInfo, BranchProbability, EdgeProbabilities, FlowGraph};
use llrm_mir::context::Context;
use llrm_mir::cycleinfo::CycleInfo;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function};
use llrm_mir::passes::{Analyses, Analysis};

use crate::branchprob;
use crate::cfg::{self, Shape};

/// `branchprob`'s odds as `BranchProbabilityInfo::getEdgeProbability(Src,
/// IndexInSuccessors)`: a successor named twice shares its edge's odds.
struct Bpi<'a> {
    function: &'a Function,
    odds: &'a branchprob::Odds,
}

impl EdgeProbabilities for Bpi<'_> {
    fn get_edge_probability(
        &self,
        src: BlockId,
        index: usize,
    ) -> BranchProbability {
        let children = self.function.children(src);
        let mut probs: Vec<BranchProbability> = children
            .iter()
            .map(|to| {
                let repeats = children.iter().filter(|one| *one == to).count() as f64;
                let odds = self.odds.probability(cfg::id(src), cfg::id(*to)).unwrap_or(0.0) / repeats;
                BranchProbability::get_branch_probability_f64(odds.clamp(0.0, 1.0))
            })
            .collect();
        BranchProbability::normalize_probabilities(&mut probs);
        probs[index]
    }
}

/// Each block's frequency (`getBlockFreq`), by block number; 0 for a block the
/// entry does not reach.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockFrequencies {
    freqs: Vec<u64>,
    entry: u64,
}

impl BlockFrequencies {
    /// `BlockFrequencyInfo::calculate` over `odds`.
    pub fn of(
        function: &Function,
        odds: &branchprob::Odds,
    ) -> Self {
        let ci = CycleInfo::compute(function);
        let bfi = BlockFrequencyInfo::calculate(function, &Bpi { function, odds }, &ci);
        let freqs =
            (0..function.block_capacity() as u32).map(|b| bfi.get_block_freq(BlockId(b)).get_frequency()).collect();
        let entry = if function.entry().is_some() { bfi.get_entry_freq().get_frequency() } else { 0 };
        Self { freqs, entry }
    }

    pub fn get_block_freq(
        &self,
        block: BlockId,
    ) -> BlockFrequency {
        BlockFrequency(self.freqs.get(block.0 as usize).copied().unwrap_or(0))
    }

    pub fn get_entry_freq(&self) -> BlockFrequency {
        BlockFrequency(self.entry)
    }

    /// `block`'s frequency over the entry's, as `isColdCallSite` and
    /// `isHotCallSite` compare them.
    pub fn relative_to_entry(
        &self,
        block: BlockId,
    ) -> f64 {
        if self.entry == 0 {
            return 0.0;
        }
        self.get_block_freq(block).get_frequency() as f64 / self.entry as f64
    }
}

impl Analysis for BlockFrequencies {
    type Result = BlockFrequencies;
    const NAME: &'static str = "block-frequencies";

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let shape = analyses.get::<Shape>(context, layout, function);
        let outer = analyses.outer();
        let odds =
            branchprob::estimated(context, &outer.metadata, &outer.globals, function, &shape, &Default::default());
        Self::of(function, &odds)
    }

    const READS_OUTER: bool = true;
}

#[cfg(test)]
#[path = "blockfreq_tests.rs"]
mod tests;
