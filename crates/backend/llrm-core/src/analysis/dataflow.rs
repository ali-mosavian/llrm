//! One solver for the backend's block-level dataflow: a worklist in layout
//! order (forward) or reverse layout order (backward), the pass supplying the
//! block's input from the outputs that flow into it and the transfer across the
//! block. A block is worked again when the output of one it depends on changed,
//! not every block each round until none does: a fact crosses one block a
//! round, so a loop nest d deep took d rounds of every block (copyprop and
//! spillforward were a third of `lir peephole` on a nest 8 deep). GCC's `df`
//! and LLVM's dataflow solvers iterate on a worklist the same way.
//!
//! `LLRM_CHECK_DATAFLOW=1` solves by rounds as the passes did and asserts the
//! same inputs and outputs: for a transfer that is monotone the fixed point
//! does not depend on the order the blocks are worked in.

use std::collections::BTreeSet;

use crate::model::lir::LirBlock;
use crate::support::hash::IndexMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// Inputs come from the predecessors' outputs; blocks are worked in layout
    /// order.
    Forward,
    /// Inputs come from the successors' outputs; blocks are worked in reverse
    /// layout order.
    Backward,
}

/// What each block's input and output came to.
pub struct Solution<S> {
    pub input: IndexMap<i64, S>,
    pub output: IndexMap<i64, S>,
}

#[cfg(test)]
thread_local! {
    /// Block transfers worked out, for a test that a block whose input is as it
    /// was is not worked again.
    pub(crate) static WORKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `nodes` solved: `initial(at)` is the output every block starts from,
/// `input(at, outputs)` a block's input from the outputs that flow into it (the
/// pass's meet, and its boundary at the entry), `transfer(at, input)` the
/// output across it. A block starts with no input; its input is worked out the
/// first time it is worked.
pub fn solve<S: Clone + PartialEq>(
    nodes: &[&LirBlock],
    direction: Direction,
    initial: impl Fn(i64) -> S,
    input: impl Fn(i64, &IndexMap<i64, S>) -> S,
    transfer: impl Fn(i64, &S) -> S,
) -> Solution<S> {
    let position: IndexMap<i64, usize> = nodes.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    // Whose input a block's output feeds.
    let mut feeds: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (index, block) in nodes.iter().enumerate() {
        for to in block.succ.iter().filter_map(|to| position.get(to)) {
            match direction {
                Direction::Forward => feeds[index].push(*to),
                Direction::Backward => feeds[*to].push(index),
            }
        }
    }
    let mut output: IndexMap<i64, S> = nodes.iter().map(|block| (block.at, initial(block.at))).collect();
    let mut entered: IndexMap<i64, S> = IndexMap::default();
    // The order blocks are worked in: the least place first, which is layout
    // order for a forward problem, its reverse for a backward.
    let place = |index: usize| match direction {
        Direction::Forward => index,
        Direction::Backward => nodes.len() - 1 - index,
    };
    let mut pending: BTreeSet<usize> = (0..nodes.len()).map(place).collect();
    while let Some(first) = pending.pop_first() {
        let index = place(first);
        let at = nodes[index].at;
        #[cfg(test)]
        WORKED.with(|count| count.set(count.get() + 1));
        let wanted = input(at, &output);
        let left = transfer(at, &wanted);
        entered.insert(at, wanted);
        if output[&at] != left {
            output.insert(at, left);
            pending.extend(feeds[index].iter().map(|dependent| place(*dependent)));
        }
    }
    if crate::support::env_set("LLRM_CHECK_DATAFLOW") {
        let mut rounds: IndexMap<i64, S> = nodes.iter().map(|block| (block.at, initial(block.at))).collect();
        let mut seen: IndexMap<i64, S> = IndexMap::default();
        let mut changed = true;
        while changed {
            changed = false;
            for first in 0..nodes.len() {
                let at = nodes[place(first)].at;
                let wanted = input(at, &rounds);
                let left = transfer(at, &wanted);
                seen.insert(at, wanted);
                if rounds[&at] != left {
                    rounds.insert(at, left);
                    changed = true;
                }
            }
        }
        assert!(
            rounds == output && seen == entered,
            "the blocks worked again where an input changed are not what working all of them round after round gives"
        );
    }
    Solution { input: entered, output }
}
