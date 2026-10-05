//! How often each block of a LIR body runs per call, and each edge is taken:
//! the one estimate every consumer asks, LLVM's `MachineBlockFrequencyInfo`
//! over `MachineBranchProbabilityInfo`.
//!
//! The edge probabilities are `LirBody::odds`, which isel made from
//! `branchprob`'s heuristics over the MIR (an edge made by splitting one
//! inherits its odds, `BlockOdds::redirected`); a loop's proven
//! trips, which induction left in `loop_trip_counts`, replace the heuristic's
//! 31 in 32; and `branchprob::propagated` turns them into frequencies.

use std::collections::BTreeSet;

use llrm_analysis::branchprob;

use crate::analysis::loops;
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
            for to in block.succ.iter().copied().collect::<BTreeSet<_>>() {
                taken.insert((block.at, to), body.odds.chance(block.at, &block.succ, to));
            }
        }
        let successors: IndexMap<i64, Vec<i64>> = blocks.iter().map(|block| (block.at, block.succ.clone())).collect();
        let mut predecessors: IndexMap<i64, Vec<i64>> = IndexMap::default();
        for block in blocks {
            for to in &block.succ {
                predecessors.entry(*to).or_default().push(block.at);
            }
        }
        let natural = loops::loops(&blocks, Some(body.entry));
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Frequency;
    use crate::model::ir::{Operation, Semantics};
    use crate::model::lir::{BlockOdds, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn insn(at: i64, op: Operation, name: &str) -> Arc<Insn> {
        let what = Semantics { name: Some(name.to_owned()), ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, 1)), Some(what), Vec::new(), Vec::new()))
    }

    fn block(at: i64, name: &str, op: Operation, succ: Vec<i64>) -> LirBlock {
        LirBlock { succ, ..LirBlock::new(at, vec![insn(at, op, name)]) }
    }

    /// entry 1 -> loop 2 (back to 2, out to 3) -> 3.
    fn counted(trips: Option<i64>, stay: f64) -> LirBody {
        let blocks = vec![
            block(1, "jmp", Operation::Jump, vec![2]),
            block(2, "jne", Operation::Branch, vec![2, 3]),
            block(3, "ret", Operation::Return, vec![]),
        ];
        let mut body = LirBody::new("f", 1, blocks, IndexMap::default(), IndexMap::default());
        for (to, probability) in [(2, stay), (3, 1.0 - stay)] {
            body.odds.taken.insert((2, to), (probability * BlockOdds::CERTAIN).round() as u32);
        }
        body.loop_trip_counts = trips.map(|count| (2, count)).into_iter().collect();
        body
    }

    /// Spill weights and the instrument read 10 per loop level, whatever the
    /// branch said; the loop's heuristic odds make it 32, and a proven count its own.
    #[test]
    fn test_a_loop_runs_as_its_odds_and_proven_trips_say_not_ten() {
        let guessed = Frequency::of(&counted(None, 124.0 / 128.0));
        assert!((guessed.block(2) - 32.0).abs() < 1e-6 && (guessed.block(3) - 1.0).abs() < 1e-6, "{} {}", guessed.block(2), guessed.block(3));
        let proven = Frequency::of(&counted(Some(5), 124.0 / 128.0));
        assert!((proven.block(2) - 5.0).abs() < 1e-6 && (proven.block(3) - 1.0).abs() < 1e-6, "{} {}", proven.block(2), proven.block(3));
    }

    /// A block almost never reached weighs almost nothing: a branch to it at 1 in a
    /// million is the cold path 10^depth gave the weight of the code before it.
    #[test]
    fn test_a_block_that_is_all_but_never_taken_weighs_nothing() {
        let blocks = vec![
            block(1, "jne", Operation::Branch, vec![2, 3]),
            block(2, "ret", Operation::Return, vec![]),
            block(3, "ret", Operation::Return, vec![]),
        ];
        let mut body = LirBody::new("f", 1, blocks, IndexMap::default(), IndexMap::default());
        for (to, probability) in [(2, 1e-6), (3, 1.0 - 1e-6)] {
            body.odds.taken.insert((1, to), (probability * BlockOdds::CERTAIN).round() as u32);
        }
        let busy = Frequency::of(&body);
        assert!(busy.block(2) < 1e-5 && busy.block(3) > 0.99, "{} {}", busy.block(2), busy.block(3));
    }
}
