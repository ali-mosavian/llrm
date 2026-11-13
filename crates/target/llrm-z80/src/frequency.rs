//! Block frequencies of an IR function: what the prices that choose between
//! variants of its machine code weigh a block's clocks by.

use std::collections::BTreeMap;

use llrm_analysis::blockfreq::BlockFrequencies;
use llrm_analysis::bpi::{BranchProbabilityInfo, Module as BpiModule};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::{LoopInfo, children};
use llrm_mir::module::{BlockId, Function, Module};
use llrm_mir::valuetracking::AssumptionCache;

/// The frequency of each block of `function` over its entry's: the pin's
/// `BlockFrequencyInfo` over `BranchProbabilityInfo`.
///
/// DEVIATE Z80-BFI-TRIPS: the pin's branch heuristics give a loop the same 32
/// trips whatever it counts, so a loop of 10 and a loop of 41 weigh alike; a
/// loop with one exit that ScalarEvolution counts (`getSmallConstantTripCount`)
/// takes its exit branch at 1 in that many, and the frequencies of the nest
/// follow by the pin's own composition. `flat` leaves the pin's. Owner z80,
/// exit 2026-11-20 or when the pin's `MachineBlockFrequencyInfo` gains trip
/// counts.
pub fn block_frequencies(
    module: &Module,
    function: &Function,
    layout: &DataLayout,
    flat: bool,
) -> Vec<(u32, f64)> {
    let bpi = BranchProbabilityInfo::of(
        BpiModule { context: &module.context, declarations: &module.globals, metadata: &module.metadata, layout },
        function,
    );
    let frequencies = if flat {
        BlockFrequencies::of_bpi(function, &bpi)
    } else {
        let mut edges = BTreeMap::new();
        for &block in function.layout() {
            let successors: Vec<BlockId> = children(function, block).collect();
            for (index, &successor) in successors.iter().enumerate() {
                let probability = bpi.get_edge_probability(function, block, index).to_f64();
                *edges.entry((i64::from(block.0), i64::from(successor.0))).or_insert(0.0) += probability;
            }
        }
        let tree = DomTree::of(function);
        let loops = LoopInfo::analyze(function, || &tree);
        let assumptions = AssumptionCache::new(module, function);
        let cx = Cx {
            context: &module.context,
            globals: &module.globals,
            metadata: &module.metadata,
            function,
            layout,
            loops: &loops,
            tree: &tree,
            assumptions: &assumptions,
        };
        let mut scev = ScalarEvolution::new();
        for one in loops.get_loops_in_preorder() {
            let [exiting] = one.get_exiting_blocks(function)[..] else { continue };
            let trips = scev.get_small_constant_trip_count(&cx, one.get_header());
            if trips == 0 {
                continue;
            }
            let successors: Vec<BlockId> = children(function, exiting).collect();
            let (leaving, staying): (Vec<BlockId>, Vec<BlockId>) = successors.iter().partition(|&&s| !one.contains(s));
            let leave = 1.0 / f64::from(trips);
            for (side, share) in [(&leaving, leave), (&staying, 1.0 - leave)] {
                let total: f64 = side.iter().map(|&s| edges[&(i64::from(exiting.0), i64::from(s.0))]).sum();
                for &s in side {
                    let key = (i64::from(exiting.0), i64::from(s.0));
                    let own = edges[&key];
                    edges.insert(key, share * if total > 0.0 { own / total } else { 1.0 / side.len() as f64 });
                }
            }
        }
        BlockFrequencies::of_probabilities(function, &edges)
    };
    function.layout().iter().map(|&block| (block.0, frequencies.relative_to_entry(block))).collect()
}
