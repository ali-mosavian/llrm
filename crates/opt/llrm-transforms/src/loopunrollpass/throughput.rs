//! DEVIATE THROUGHPUT (owner x86-m64, exit 2026-12-20): what a full unroll saves on a core that overlaps
//! instructions. A trip costs the larger of two bounds, as the pin's cost model reads a loop:
//!
//! * the resource bound, the largest over the model's processor resources of the cycles the trip's
//!   instructions put on one over its units, the issue width one resource of micro-ops
//!   (`MachineTraceMetrics::Trace::getResourceLength`, MachineTraceMetrics.cpp);
//! * the chain bound, the critical path of the loop's dependences over two iterations at the instructions'
//!   latencies, with infinite resources (`SelectOptimize::computeLoopCosts`, SelectOptimize.cpp:1305): the
//!   rise of the longest path from the first iteration to the second.
//!
//! Fewer instructions per trip shorten a resource-bound loop only: the instructions an unroll removes (the
//! counter, the exit test, folded units) leave a chain-bound loop as long as it was.
//!
//! The pin's `shouldFullUnroll` counts instructions and prices nothing (UP1 is the deviation that prices);
//! the table is the target's, in thousandths of a clock (`Machine::throughput`).
//!
//! The model leaves the pin's choice only on a priced net gain: it declines a pin unroll when the clocks it saves
//! do not pay its bytes at the target's rate, and takes one the threshold refuses when they do. Where the copies
//! make the loop's stores into a local array immediates for a reader that folds (UP1-REG), the array holds no
//! register, so the pin's unroll stands. Exit 2026-12-20: vector MIR, which folds what this model cannot
//! (`dot` -O3 m64: 817 B here, clang 26 B).

use llrm_mir::dense::IdMap;
use llrm_mir::module::ValueDef;
use llrm_mir::target::{OperationCosts, Throughput};

use super::*;
use crate::loopidioms::{counted, trip_clocks};

/// One trip of a loop: the milli-cycles it puts on each of the model's resources, and its chain.
pub(super) struct Trip {
    pub load: Vec<i64>,
    pub chain: i64,
}

/// The time the loads `load` take on the model's resources, milli-clocks: the busiest resource over its units.
pub(super) fn resource_bound(
    tp: &Throughput,
    load: &[i64],
) -> i64 {
    tp.resources.iter().zip(load).map(|(resource, cycles)| cycles / resource.units.max(1)).max().unwrap_or(0)
}

impl Trip {
    /// The bound a trip runs at.
    pub(super) fn bound(
        &self,
        tp: &Throughput,
    ) -> i64 {
        resource_bound(tp, &self.load).max(self.chain)
    }
}

/// `weight` is the copies of a block the inner loops' unrolls make (a projected nest), 1 otherwise.
pub(super) fn trip(
    run: &Run,
    blocks: &[BlockId],
    tp: &Throughput,
    weight: &dyn Fn(BlockId) -> i64,
) -> Trip {
    let load = tp
        .resources
        .iter()
        .map(|resource| {
            blocks
                .iter()
                .map(|block| {
                    weight(*block)
                        * trip_clocks(run.context, run.layout, run.function, run.callees, std::iter::once(block), &resource.costs)
                })
                .sum()
        })
        .collect();
    Trip { load, chain: chain(run, blocks, tp) }
}

/// What `operation` of each of `costs` fields a loop's control and folded work remove: `pick` reads the one operation.
pub(super) fn per_resource(
    tp: &Throughput,
    pick: &dyn Fn(&OperationCosts) -> i64,
) -> Vec<i64> {
    tp.resources.iter().map(|resource| pick(&resource.costs)).collect()
}

/// The critical path of one trip: `computeLoopCosts` over two iterations, a value's cost the instruction's latency
/// plus the largest cost among its operands, a loop-carried operand taking the previous iteration's.
fn chain(
    run: &Run,
    blocks: &[BlockId],
    tp: &Throughput,
) -> i64 {
    let function = &*run.function;
    let mut costs: IdMap<InstId, i64> = IdMap::new();
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

/// Milli-clocks `trips` trips of `trip` save when the unroll removes `removed` milli-cycles from each resource over
/// the whole run: a chain-bound loop keeps its time.
pub(super) fn saved(
    tp: &Throughput,
    trip: &Trip,
    trips: i64,
    removed: &[i64],
) -> i64 {
    let rolled = trips * trip.bound(tp);
    let left: Vec<i64> = trip.load.iter().zip(removed).map(|(load, removed)| (trips * load - removed).max(0)).collect();
    let unrolled = resource_bound(tp, &left).max(trips * trip.chain);
    (rolled - unrolled).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use llrm_mir::target::Resource;

    fn model() -> Throughput {
        let costs = OperationCosts::default();
        Throughput { latency: costs.clone(), resources: vec![Resource { units: 4, costs: costs.clone() }, Resource { units: 1, costs }] }
    }

    /// A trip bound by its chain keeps its time whatever the unroll removes of its instructions: the 168 clocks a
    /// 12-trip loop of calls (huffman_decode's `report(decode(t))`) was credited for its counter and exit test.
    #[test]
    fn a_chain_bound_trip_saves_nothing_and_a_resource_bound_trip_saves_what_is_removed() {
        let tp = model();
        let chained = Trip { load: vec![8000, 1000], chain: 4000 };
        assert_eq!(saved(&tp, &chained, 12, &[12 * 2000, 0]), 0, "issue 2 clk, chain 4: the chain is the time");
        let issue_bound = Trip { load: vec![16000, 1000], chain: 1000 };
        assert_eq!(saved(&tp, &issue_bound, 12, &[12 * 12000, 0]), 12 * 3000, "issue 4 clk to 1, the 1 clk chain");
    }
}
