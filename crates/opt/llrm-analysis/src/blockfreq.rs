//! `BlockFrequencyInfo` of a MIR function: LLVM's `BlockFrequencyInfoImpl`
//! (`llrm_mir::blockfrequency`) over `BranchProbabilityInfo` (`crate::bpi`),
//! as `BlockFrequencyInfoWrapperPass` runs it. `trip_adjusted` and
//! `of_probabilities` serve llrm's own estimates, which let a loop's proven
//! trips set its exit (`profit`).
//!
//! # Used by
//! `inlinecost` (cold and hot call sites, D1), `profit`.
//!
//! # Example
//! A diamond whose branch weighs 3 to 1 gives the arms 0.75 and 0.25 of the
//! entry's frequency, and the join the entry's:
//!
//! ```text
//! entry: br i1 %c, label %a, label %b   ; !{branch_weights, 3, 1}
//! ; entry 1.0, a 0.75, b 0.25, join 1.0
//! ```

use llrm_mir::blockfrequency::{BlockFrequency, BlockFrequencyInfo, BranchProbability, EdgeProbabilities, FlowGraph};
use llrm_mir::context::Context;
use llrm_mir::cycleinfo::CycleInfo;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function};
use llrm_mir::passes::{Analyses, Analysis};

use crate::cfg;

/// Edge probabilities by `(from, to)` block ids as
/// `BranchProbabilityInfo::getEdgeProbability(Src, IndexInSuccessors)`: a
/// successor named twice shares its edge's probability.
struct Bpi<'a, P: Fn(i64, i64) -> Option<f64>> {
    function: &'a Function,
    probability: P,
}

impl<P: Fn(i64, i64) -> Option<f64>> EdgeProbabilities for Bpi<'_, P> {
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
                let odds = (self.probability)(cfg::id(src), cfg::id(*to)).unwrap_or(0.0) / repeats;
                BranchProbability::get_branch_probability_f64(odds.clamp(0.0, 1.0))
            })
            .collect();
        BranchProbability::normalize_probabilities(&mut probs);
        probs[index]
    }
}

/// A loop of `natural` whose trips are proven (`trips`, by header) leaves by
/// its exit test one time in `trips + 1 - tested` (`tested` is 1 for a test
/// after the trip, at a latch, 0 for one before it, at the header): a lone
/// exit wherever it is, of several those at the header or a latch; the
/// block's other edges keep their shares. LLVM's
/// probabilities know no trip count: this is for llrm's own estimates
/// (`profit`'s weights, the backend's `executed`).
pub fn trip_adjusted(
    function: &Function,
    natural: &[crate::graph::loops::Loop],
    trips: &std::collections::BTreeMap<i64, i64>,
    probabilities: &std::collections::BTreeMap<(i64, i64), f64>,
) -> std::collections::BTreeMap<(i64, i64), f64> {
    let mut edges = probabilities.clone();
    let successors =
        |from: i64| -> Vec<i64> { function.successors(cfg::block(from)).into_iter().map(cfg::id).collect() };
    for one in natural {
        let Some(&count) = trips.get(&one.header) else { continue };
        let exiting: Vec<i64> = function
            .layout()
            .iter()
            .map(|&block| cfg::id(block))
            .filter(|from| one.body.contains(from) && successors(*from).iter().any(|to| !one.body.contains(to)))
            .collect();
        for &from in &exiting {
            // A lone exit is the loop's test wherever it is; of several, only
            // those at the header or a latch count trips.
            let tested = if one.latches.contains(&from) {
                1
            } else if from == one.header {
                0
            } else if exiting.len() == 1 {
                1
            } else {
                continue;
            };
            let out = 1.0 / (count + 1 - tested).max(1) as f64;
            let (leaving, staying): (Vec<i64>, Vec<i64>) =
                successors(from).into_iter().partition(|to| !one.body.contains(to));
            for (side, share) in [(&leaving, out), (&staying, 1.0 - out)] {
                let total: f64 = side.iter().map(|to| probabilities.get(&(from, *to)).copied().unwrap_or(0.0)).sum();
                for to in side {
                    let p = probabilities.get(&(from, *to)).copied().unwrap_or(0.0);
                    edges.insert((from, *to), share * if total > 0.0 { p / total } else { 1.0 / side.len() as f64 });
                }
            }
        }
    }
    edges
}

/// Each block's frequency (`getBlockFreq`), by block number; 0 for a block the
/// entry does not reach.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockFrequencies {
    freqs: Vec<u64>,
    entry: u64,
}

impl BlockFrequencies {
    /// `BlockFrequencyInfo::calculate` over `probabilities`, by `(from, to)`
    /// block ids.
    pub fn of_probabilities(
        function: &Function,
        probabilities: &std::collections::BTreeMap<(i64, i64), f64>,
    ) -> Self {
        let ci = CycleInfo::compute(function);
        let bfi = BlockFrequencyInfo::calculate(
            function,
            &Bpi { function, probability: |from, to| probabilities.get(&(from, to)).copied() },
            &ci,
        );
        let freqs =
            (0..function.block_capacity() as u32).map(|b| bfi.get_block_freq(BlockId(b)).get_frequency()).collect();
        let entry = if function.entry().is_some() { bfi.get_entry_freq().get_frequency() } else { 0 };
        Self { freqs, entry }
    }

    /// `BlockFrequencyInfo::calculate` over `bpi`, as LLVM's
    /// `BlockFrequencyAnalysis` runs it.
    pub fn of_bpi(
        function: &Function,
        bpi: &crate::bpi::BranchProbabilityInfo,
    ) -> Self {
        let ci = CycleInfo::compute(function);
        let bfi = BlockFrequencyInfo::calculate(function, &bpi.edges(function), &ci);
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
        let bpi = analyses.get::<crate::bpi::BranchProbabilityInfo>(context, layout, function);
        Self::of_bpi(function, &bpi)
    }

    const READS_OUTER: bool = true;
}

#[cfg(test)]
#[path = "blockfreq_tests.rs"]
mod tests;
