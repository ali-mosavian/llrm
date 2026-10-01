//! How often each block of a LIR body runs per call, and each edge is taken:
//! the one estimate every consumer asks, LLVM's `MachineBlockFrequencyInfo`
//! over `MachineBranchProbabilityInfo`.
//!
//! The edge probabilities are `LirBody::odds`, which isel made from
//! `branchprob`'s heuristics over the MIR (an edge made since, as a split
//! one, takes what its block's isel edges leave, evenly); a loop's proven
//! trips, which induction left in `loop_trip_counts`, replace the heuristic's
//! 31 in 32; and `branchprob::propagated` turns them into frequencies.

use std::collections::BTreeSet;

use llrm_analysis::branchprob;

use crate::analysis::{intervals, loops};
use crate::model::lir::{LirBlock, LirBody};
use crate::support::hash::IndexMap;

pub struct Frequency {
    block: IndexMap<i64, f64>,
    taken: IndexMap<(i64, i64), f64>,
}

impl Frequency {
    /// `body`'s blocks' frequencies.
    pub fn of(body: &LirBody) -> Self {
        Self::over(body, &body.blocks)
    }

    /// `blocks`, a later arrangement of `body`'s, on `body`'s odds and trips.
    pub fn over(body: &LirBody, blocks: &[LirBlock]) -> Self {
        let mut taken = IndexMap::default();
        for block in blocks {
            let succ: Vec<i64> = block.succ.iter().copied().collect::<BTreeSet<_>>().into_iter().collect();
            let known: f64 = succ.iter().filter_map(|to| body.odds.probability(block.at, *to)).sum();
            let unknown = succ.iter().filter(|to| body.odds.probability(block.at, **to).is_none()).count();
            for to in &succ {
                // An edge made after isel carries what its block's isel edges do not.
                let share = match body.odds.probability(block.at, *to) {
                    Some(probability) => probability,
                    None => (1.0 - known).max(0.0) / unknown as f64,
                };
                taken.insert((block.at, *to), if succ.len() == 1 { 1.0 } else { share });
            }
        }
        let successors: IndexMap<i64, Vec<i64>> = blocks.iter().map(|block| (block.at, block.succ.clone())).collect();
        let mut predecessors: IndexMap<i64, Vec<i64>> = IndexMap::default();
        for block in blocks {
            for to in &block.succ {
                predecessors.entry(*to).or_default().push(block.at);
            }
        }
        let natural = loops::loops(&intervals::_graph(blocks), Some(body.entry));
        let trips: IndexMap<i64, i64> = body.loop_trip_counts.iter().copied().collect();
        let cycles: Vec<branchprob::Cycle> = natural
            .iter()
            .map(|one| branchprob::Cycle { header: one.header, latches: &one.latches, body: &one.body, trips: trips.get(&one.header).copied() })
            .collect();
        let order = branchprob::reverse_postorder_of(body.entry, &|at| successors.get(&at).cloned().unwrap_or_default());
        let block = branchprob::propagated(
            &order,
            &|at| predecessors.get(&at).cloned().unwrap_or_default(),
            &|at| successors.get(&at).cloned().unwrap_or_default(),
            &cycles,
            &|from, to| taken.get(&(from, to)).copied().unwrap_or(0.0),
        );
        Self { block: block.into_iter().collect(), taken }
    }

    /// Per call; zero for a block the entry does not reach.
    pub fn block(&self, at: i64) -> f64 {
        self.block.get(&at).copied().unwrap_or(0.0)
    }

    /// How often `from` goes to `to`, per call.
    pub fn edge(&self, from: i64, to: i64) -> f64 {
        self.block(from) * self.taken.get(&(from, to)).copied().unwrap_or(0.0)
    }
}
