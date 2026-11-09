//! DEVIATE RU-REC (owner x86-m64; exit 2026-11-20): a loop is not unrolled at
//! run time when its loop-carried dependence already takes as long as the
//! core needs to issue one trip.
//!
//! The pin's `getUnrollingPreferences` sets `UP.Runtime` for any loop that fits
//! the loop buffer (BasicTTIImpl.h L738-803) and asks nothing of the loop.
//! Unrolling only removes the trip's counter and branch; with the time per trip
//! set by a chain of dependent operations, the copies run no sooner, and the
//! guard, the split of the count and the remainder loop are code for nothing
//! (floats -O2 394 B against gcc's 207 for the same clocks).
//!
//! The chain is the pin's own estimate of a loop's critical path,
//! `SelectOptimize::computeLoopCosts`: two walks of the body, each operation
//! costing its latency on top of its dearest operand, the second walk seeing
//! the first's values through the header phis. The growth of the longest path
//! between the walks is the latency a trip adds. Only a single-block loop is
//! priced: its operations all run each trip, so the sums are the trip's.
//!
//! Known limits, not claims: a chain through memory (`a[i] = a[i - 1] * c`)
//! shows no growth and keeps the pin's unroll, and so does a loop of several
//! blocks. Latencies and throughputs are the `Tti`'s, as the pin's.
//!
//! <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1305>

use llrm_mir::dense::IdMap;
use llrm_mir::module::{InstId, ValueDef};

use super::*;

/// Whether the runtime unroll of `lp` is vetoed: the target asks it
/// (`Tuning::unroll_recurrence_veto`), states its issue width, and the loop is
/// a single block whose chain is at least its throughput cost over that width.
pub(super) fn recurrence_bound(
    run: &Run,
    lp: Loop,
) -> bool {
    let width = i64::from(run.tti.issue_width());
    if width == 0 || !run.tti.vetoes_recurrence_bound_unroll() {
        return false;
    }
    let &[block] = lp.get_blocks() else { return false };
    let price = |inst: InstId, kind: CostKind| {
        let cost = instruction_cost(run.tti, run.context, run.layout, run.globals, run.function, inst, kind);
        cost.valid.then_some(cost.value)
    };
    let chain = chain_growth(run.function, block, |inst| price(inst, CostKind::Latency));
    let issue = run
        .function
        .iter_block(block)
        .map(|inst| price(inst, CostKind::RecipThroughput))
        .sum::<Option<i64>>();
    let (Some(chain), Some(issue)) = (chain, issue) else { return false };
    chain * width >= issue
}

/// The longest dependence path through two walks of `block`, the second minus
/// the first: what one more trip adds. `None` when an operation has no price.
fn chain_growth(
    function: &Function,
    block: BlockId,
    latency: impl Fn(InstId) -> Option<i64>,
) -> Option<i64> {
    let mut path: IdMap<InstId, i64> = IdMap::new();
    let mut longest = [0i64; 2];
    for longest in &mut longest {
        for inst in function.iter_block(block) {
            let operands = function
                .instruction(inst)
                .operands
                .iter()
                .filter_map(|&operand| match operand {
                    Operand::Value(value) => match function.value(value).def {
                        ValueDef::Instruction(def) => path.get(&def).copied(),
                        _ => None,
                    },
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            let here = operands + latency(inst)?;
            path.insert(inst, here);
            *longest = (*longest).max(here);
        }
    }
    Some(longest[1] - longest[0])
}
