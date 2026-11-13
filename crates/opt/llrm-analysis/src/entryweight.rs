//! How often a function runs: the frequency of a block over its function's
//! entry (`BlockFrequencyInfo` over `BranchProbabilityInfo`, as the inliner's
//! `GetBFI` has it) and, from the direct calls to a function, how many times it
//! runs per run of the program.

use std::collections::BTreeSet;

use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, GlobalKind, InstId, Linkage, Module};
use llrm_support::hash::HashMap;

use crate::blockfreq::BlockFrequencies;

fn local(linkage: Linkage) -> bool {
    matches!(linkage, Linkage::Internal | Linkage::Private)
}

/// `BlockFrequencyInfo` of `caller` as the inliner's `GetBFI` gives it: over
/// `BranchProbabilityInfo` (D1).
pub fn caller_frequencies(
    module: &Module,
    layout: &DataLayout,
    caller: &Function,
) -> BlockFrequencies {
    let bpi = crate::bpi::BranchProbabilityInfo::of(
        crate::bpi::Module {
            context: &module.context,
            declarations: &module.globals,
            metadata: &module.metadata,
            layout,
        },
        caller,
    );
    BlockFrequencies::of_bpi(caller, &bpi)
}

/// `block`'s frequency over the entry's; none where the entry's is 0.
pub fn relative_to_entry(
    frequencies: &BlockFrequencies,
    block: BlockId,
) -> Option<f64> {
    (frequencies.get_entry_freq().get_frequency() > 0).then(|| frequencies.relative_to_entry(block))
}

/// How many times `function` runs per run of the program, estimated from the
/// calls to it: the sum over its direct call sites of the site's frequency
/// over its function's entry, times that function's own count, up to
/// `ENTRY_DEPTH` calls. A function with an outside caller (not local, or its
/// address taken) or none runs at least once. Capped at `ENTRY_CAP`.
pub fn entry_weight(
    module: &Module,
    layout: &DataLayout,
    function: GlobalId,
    addressed: &BTreeSet<GlobalId>,
) -> f64 {
    const ENTRY_DEPTH: usize = 8;
    const ENTRY_CAP: f64 = 256.0;
    let mut frequencies: HashMap<GlobalId, BlockFrequencies> = HashMap::default();
    fn weight(
        module: &Module,
        layout: &DataLayout,
        function: GlobalId,
        addressed: &BTreeSet<GlobalId>,
        depth: usize,
        visiting: &mut Vec<GlobalId>,
        frequencies: &mut HashMap<GlobalId, BlockFrequencies>,
    ) -> f64 {
        let outside = !local(module.global(function).linkage) || addressed.contains(&function);
        if depth == 0 {
            return 1.0;
        }
        visiting.push(function);
        let mut total = 0.0;
        for (at, global) in module.globals.iter().enumerate() {
            let caller_id = GlobalId(at as u32);
            let GlobalKind::Function(caller) = &global.kind else { continue };
            if visiting.contains(&caller_id) || caller.layout().is_empty() {
                continue;
            }
            let sites: Vec<InstId> = caller
                .walk()
                .map(|(_, inst)| inst)
                .filter(|&inst| llrm_mir::memory::callee(&module.context, caller, inst) == Some(function))
                .collect();
            if sites.is_empty() {
                continue;
            }
            let inner = weight(module, layout, caller_id, addressed, depth - 1, visiting, frequencies);
            let table = frequencies.entry(caller_id).or_insert_with(|| caller_frequencies(module, layout, caller));
            for call in sites {
                let local = caller.parent(call).and_then(|block| relative_to_entry(table, block)).unwrap_or(1.0);
                total += local * inner;
            }
        }
        visiting.pop();
        if outside || total == 0.0 { total.max(1.0) } else { total }
    }
    weight(module, layout, function, addressed, ENTRY_DEPTH, &mut Vec::new(), &mut frequencies).min(ENTRY_CAP)
}
