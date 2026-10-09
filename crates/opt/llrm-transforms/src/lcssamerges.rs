//! Adapted from llrm-core's `optimize/lcssamerges.rs`, the port of
//! `qbopt/optimize/lcssamerges.py`: a loop with several exits is closed by
//! a phi at each exit, merged by phis where the exits' paths join.
//!
//! Joins are resolved to the exit or join block that supplies them before
//! any phi is made, so a refused merge creates nothing. Python's
//! `ValueError`s are the `Err` text.
//!
//! Ported tests are in `lcssamerges_tests.rs`. Stays behind, reading a BC
//! corpus: `test_compiled_early_exit_accumulator_is_closed`
//! (`lcmerge-p-g2.obj`; ignored there too).

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::graph::loops::{self, Loop};
use llrm_analysis::{cfg, ssa};
use llrm_mir::module::{Function, Operand, ValueId};

use crate::edges;
use crate::lcssa::{arms, definitions, exit_phi, from_arms, operations, place_phi};

/// Whether anything changed.
pub fn closed(
    function: &mut Function,
    loop_: &Loop,
) -> Result<bool, String> {
    let graph = cfg::graph(function);
    let entry = function.entry().map(cfg::id);
    let predecessors = loops::predecessors(&graph);
    let dominance = cfg::Dominance::of(function);
    let exits = graph
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .flat_map(|block| block.succ.iter().copied())
        .filter(|at| !loop_.body.contains(at))
        .collect::<BTreeSet<_>>();
    if exits
        .iter()
        .any(|at| predecessors.get(at).is_none_or(|parents| parents.is_empty() || !parents.is_subset(&loop_.body)))
    {
        return Ok(false);
    }
    let definitions = definitions(function, &loop_.body);
    let mut sites: BTreeMap<ValueId, BTreeSet<i64>> = BTreeMap::new();
    for block in graph.iter().filter(|block| !loop_.body.contains(&block.at)) {
        for inst in operations(function, cfg::block(block.at)) {
            for operand in &function.instruction(inst).operands {
                if let Operand::Value(value) = operand
                    && definitions.contains_key(value)
                {
                    sites.entry(*value).or_default().insert(block.at);
                }
            }
        }
        for phi in edges::phis(function, cfg::block(block.at)) {
            for (value, parent) in arms(function, phi) {
                if let Operand::Value(value) = value
                    && !loop_.body.contains(&cfg::id(parent))
                    && definitions.contains_key(&value)
                {
                    sites.entry(value).or_default().insert(cfg::id(parent));
                }
            }
        }
    }
    let mut changed = false;
    for (&value, value_sites) in &sites {
        let available = exits
            .iter()
            .copied()
            .filter(|at| predecessors[at].iter().all(|&parent| dominance.dominates(definitions[&value], parent)))
            .collect::<BTreeSet<_>>();
        let mut needed = BTreeSet::new();
        let mut pending = value_sites.iter().copied().collect::<Vec<_>>();
        let mut broke = false;
        while let Some(at) = pending.pop() {
            if needed.contains(&at) {
                continue;
            }
            if loop_.body.contains(&at) || Some(at) == entry || predecessors.get(&at).is_none_or(BTreeSet::is_empty) {
                broke = true;
                break;
            }
            needed.insert(at);
            if !available.contains(&at) {
                pending.extend(predecessors[&at].iter().copied());
            }
        }
        if !broke {
            let reached = available.intersection(&needed).copied().collect();
            changed |= _merged(function, value, &needed, &reached, &predecessors)?;
        }
    }
    Ok(changed)
}

/// `value` read through a phi at each of `exits` and at each join of their
/// paths within `needed`; whether it was.
pub fn _merged(
    function: &mut Function,
    value: ValueId,
    needed: &BTreeSet<i64>,
    exits: &BTreeSet<i64>,
    predecessors: &BTreeMap<i64, BTreeSet<i64>>,
) -> Result<bool, String> {
    let mut reaching = needed
        .iter()
        .map(|&at| (at, if exits.contains(&at) { BTreeSet::from([at]) } else { BTreeSet::new() }))
        .collect::<BTreeMap<_, _>>();
    loop {
        let changed = needed
            .iter()
            .map(|&at| {
                let now = if exits.contains(&at) {
                    reaching[&at].clone()
                } else {
                    predecessors[&at].iter().flat_map(|parent| reaching[parent].iter().copied()).collect()
                };
                (at, now)
            })
            .collect::<BTreeMap<_, _>>();
        if changed == reaching {
            break;
        }
        reaching = changed;
    }
    if reaching.values().any(BTreeSet::is_empty) {
        return Ok(false);
    }
    let mut joins = exits.clone();
    joins.extend(needed.iter().copied().filter(|at| reaching[at].len() > 1 && predecessors[at].len() > 1));
    // Which join's phi each needed block reads.
    let mut supplier = joins.iter().map(|&at| (at, at)).collect::<BTreeMap<_, _>>();
    let mut pending = needed.difference(&joins).copied().collect::<BTreeSet<_>>();
    while !pending.is_empty() {
        let mut changed = BTreeSet::new();
        for &at in &pending {
            if reaching[&at].len() == 1 {
                let only = *reaching[&at].first().expect("one");
                supplier.insert(at, supplier[&only]);
            } else {
                let parents = &predecessors[&at];
                if parents.len() > 1 {
                    return Err("too many values to unpack (expected 1)".to_owned());
                }
                let Some(parent) = parents.first() else {
                    return Err("not enough values to unpack (expected 1, got 0)".to_owned());
                };
                let Some(&found) = supplier.get(parent) else {
                    continue;
                };
                supplier.insert(at, found);
            }
            changed.insert(at);
        }
        if changed.is_empty() {
            return Ok(false);
        }
        pending = pending.difference(&changed).copied().collect();
    }
    let phis = joins.iter().map(|&at| (at, exit_phi(function, value))).collect::<BTreeMap<_, _>>();
    let result = |function: &Function, at: i64| {
        Operand::Value(function.instruction(phis[&supplier[&at]]).result.expect("a phi's value"))
    };
    let replacements = supplier.keys().map(|&at| (at, result(function, at))).collect::<BTreeMap<_, _>>();
    for (&at, &phi) in &phis {
        let incoming = predecessors[&at]
            .iter()
            .map(|&parent| {
                (if exits.contains(&at) { Operand::Value(value) } else { replacements[&parent] }, cfg::block(parent))
            })
            .collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&incoming));
    }
    let swap = |at: i64| BTreeMap::from([(value, replacements[&at])]);
    for block in function.layout().to_vec() {
        for phi in edges::phis(function, block) {
            let incoming = arms(function, phi)
                .into_iter()
                .map(|(incoming, parent)| match replacements.get(&cfg::id(parent)) {
                    Some(&replacement) if incoming == Operand::Value(value) => (replacement, parent),
                    _ => (incoming, parent),
                })
                .collect::<Vec<_>>();
            if from_arms(&incoming) != function.instruction(phi).operands {
                function.set_operands(phi, from_arms(&incoming));
            }
        }
        if replacements.contains_key(&cfg::id(block)) {
            for inst in operations(function, block) {
                let operands = ssa::substituted(function.instruction(inst), &swap(cfg::id(block)))
                    .map_err(|error| error.to_string())?;
                if operands != function.instruction(inst).operands {
                    function.set_operands(inst, operands);
                }
            }
        }
    }
    for (&at, &phi) in &phis {
        place_phi(function, cfg::block(at), phi)?;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "lcssamerges_tests.rs"]
mod tests;
