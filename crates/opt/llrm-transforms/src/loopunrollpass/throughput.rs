//! DEVIATE THROUGHPUT (owner x86-m64, exit 2026-12-20): what a full unroll saves on a core that overlaps
//! instructions. A trip costs the larger of two bounds, as the pin's cost model reads a loop:
//!
//! * the resource bound, the sum of the instructions' reciprocal throughputs
//!   (`MCSchedModel::getReciprocalThroughput`, MCSchedule.cpp:114, summed as `LoopVectorize` sums
//!   `TCK_RecipThroughput`);
//! * the chain bound, the critical path of the loop's dependences over two iterations at the instructions'
//!   latencies, with infinite resources (`SelectOptimize::computeLoopCosts`, SelectOptimize.cpp:1305): the
//!   rise of the longest path from the first iteration to the second.
//!
//! Fewer instructions per trip shorten a resource-bound loop only: the instructions an unroll removes (the
//! counter, the exit test, folded units) leave a chain-bound loop as long as it was.
//!
//! The pin's `shouldFullUnroll` counts instructions and prices nothing (UP1 is the deviation that prices);
//! the table is the target's, in thousandths of a clock (`Machine::throughput`).

use std::collections::HashMap;

use llrm_mir::module::ValueDef;
use llrm_mir::target::Throughput;

use super::*;
use crate::loopidioms::{counted, trip_clocks};

/// One trip of a loop, in milli-clocks.
pub(super) struct Trip {
    pub resource: i64,
    pub chain: i64,
}

impl Trip {
    /// The bound the trip runs at.
    pub(super) fn bound(&self) -> i64 {
        self.resource.max(self.chain)
    }
}

/// `weight` is the copies of a block the inner loops' unrolls make (a projected nest), 1 otherwise.
pub(super) fn trip(
    run: &Run,
    blocks: &[BlockId],
    tp: &Throughput,
    weight: &dyn Fn(BlockId) -> i64,
) -> Trip {
    let once = |block: &BlockId| trip_clocks(run.context, run.layout, run.function, run.callees, std::iter::once(block), &tp.recip);
    Trip { resource: blocks.iter().map(|block| weight(*block) * once(block)).sum(), chain: chain(run, blocks, tp) }
}

/// The critical path of one trip: `computeLoopCosts` over two iterations, a value's cost the instruction's latency
/// plus the largest cost among its operands, a loop-carried operand taking the previous iteration's.
fn chain(
    run: &Run,
    blocks: &[BlockId],
    tp: &Throughput,
) -> i64 {
    let function = &*run.function;
    let mut costs: HashMap<InstId, i64> = HashMap::new();
    let mut longest = [0_i64; 2];
    for longest in &mut longest {
        for &block in blocks {
            for inst in function.iter_block(block) {
                let one = function.instruction(inst);
                let before = one
                    .operands
                    .iter()
                    .filter_map(|operand| match operand {
                        Operand::Value(value) => match function.value(*value).def {
                            ValueDef::Instruction(def) => costs.get(&def).copied(),
                            ValueDef::Argument(_) => None,
                        },
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0);
                let latency = if counted(function, inst) {
                    crate::profit::operation(run.context, run.layout, function, run.callees, inst, &tp.latency).unwrap_or(tp.latency.add)
                } else {
                    0
                };
                let cost = before + latency;
                costs.insert(inst, cost);
                *longest = (*longest).max(cost);
            }
        }
    }
    (longest[1] - longest[0]).max(0)
}

/// Milli-clocks `trips` trips of `trip` save when the unroll removes `removed` milli-clocks of resources
/// from the whole run: a chain-bound loop keeps its time.
pub(super) fn saved(
    trip: &Trip,
    trips: i64,
    removed: i64,
) -> i64 {
    let rolled = trips * trip.bound();
    let unrolled = (trips * trip.resource - removed).max(trips * trip.chain).max(0);
    (rolled - unrolled).max(0)
}
