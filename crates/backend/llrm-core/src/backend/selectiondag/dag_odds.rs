//! The odds of each edge of a selected body, from the MIR's branch
//! probabilities, as the old selector's `odds` gives them.
//!
//! # How it works
//! `BranchProbabilityInfo` gives each MIR edge `B -> T` its probability. A LIR
//! block that leaves for other MIR blocks takes the probability of its edge to
//! each. A block of the chain a lowering made inside `B` (a switch's compares,
//! a merged condition) names some MIR targets and some blocks of the chain: the
//! targets keep their probability and the chain's own edges share what is left.
//! Blocks with no edge to another MIR block are left without odds, which read
//! as even.
//!
//! ```text
//!   B: switch -> 7 cases + default   cmp x,3 ; je case3 (1/8)  else next compare (7/8)
//! ```
//!
//! # Example
//! `bench/switch/switch.c` under `--isel=dag`: the compare of a case leaves for
//! the case with 1/8 and for the next compare with 7/8, so the miss falls
//! through; every edge was even before.
//!
//! # Used by
//! `dag_function::selected_in`, which sets the body's `odds`. Closes
//! DEVIATE SD403's missing odds; trip counts are not carried.

use std::collections::HashMap;
use std::ops::Range;

use indexmap::IndexMap;
use llrm_mir::module::{BlockId, Function};

use crate::model::lir::BlockOdds;

/// The odds of each edge out of the LIR blocks `at`/`succ` (the blocks of
/// `made`'s ranges, in order) of `function`.
pub(super) fn of(
    bpi: &llrm_analysis::bpi::BranchProbabilityInfo,
    function: &Function,
    made: &HashMap<u32, Range<usize>>,
    block_at: &HashMap<u32, i64>,
    at: &[i64],
    succ: &[Vec<i64>],
) -> BlockOdds {
    let mut odds = BlockOdds::default();
    for &block in function.layout() {
        let Some(range) = made.get(&block.0) else { continue };
        let taken: IndexMap<i64, f64> = function
            .successors(block)
            .into_iter()
            .filter_map(|to: BlockId| {
                block_at.get(&to.0).map(|place| (*place, bpi.get_edge_probability_to(function, block, to).to_f64()))
            })
            .collect();
        for index in range.clone() {
            let leaving = &succ[index];
            if leaving.len() < 2 {
                continue;
            }
            let known: f64 = leaving.iter().filter_map(|to| taken.get(to)).sum();
            let inner = leaving.iter().filter(|to| !taken.contains_key(*to)).count();
            if known == 0.0 && inner == leaving.len() {
                continue;
            }
            let weight = |to: &i64| taken.get(to).copied().unwrap_or((1.0 - known).max(0.0) / inner.max(1) as f64);
            let total: f64 = leaving.iter().map(weight).sum();
            if total <= 0.0 {
                continue;
            }
            for to in leaving {
                odds.taken.insert(
                    (at[index], *to),
                    (weight(to) / total * BlockOdds::CERTAIN).round().min(f64::from(u32::MAX)) as u32,
                );
            }
        }
    }
    odds
}
