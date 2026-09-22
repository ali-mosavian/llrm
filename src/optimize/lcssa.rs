//! Port of `qbopt/optimize/lcssa.py`.
//!
//! Python's `ValueError` from `ssa.substituted` is the `Err` text.

use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;

use crate::analysis::loops::{self, Loop};
use crate::analysis::ssa;
use crate::model::mir::{MirBlock, MirBody, Phi, Value};
use crate::model::passes::MIRTransform;
use crate::optimize::lcssamerges;

pub(crate) struct LoopClosedSSA;

impl MIRTransform for LoopClosedSSA {
    fn class_name(&self) -> &'static str {
        "LoopClosedSSA"
    }

    fn name(&self) -> &str {
        "lcssa"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        closed(&body)
    }
}

/// Return `body` with every supported natural loop in closed SSA form.
pub(crate) fn closed(body: &MirBody) -> Result<MirBody, String> {
    let mut result = body.clone();
    // loops() deliberately returns inner loops first.  Closing an inner loop
    // first makes its exit value an ordinary definition in an enclosing loop.
    for loop_ in loops::loops(&result.blocks, Some(result.entry)) {
        result = _closed_loop(&result, &loop_)?;
    }
    Ok(result)
}

pub(crate) fn _closed_loop(body: &MirBody, loop_: &Loop) -> Result<MirBody, String> {
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let exiting = body
        .blocks
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .flat_map(|block| block.succ.iter().map(move |&successor| (block.at, successor)))
        .filter(|(_, successor)| blocks.contains_key(successor) && !loop_.body.contains(successor))
        .collect::<Vec<_>>();
    let exits = exiting.iter().map(|&(_, target)| target).collect::<BTreeSet<_>>();
    if exits.len() > 1 {
        return lcssamerges::closed(body, loop_);
    }
    if exits.is_empty() {
        return Ok(body.clone());
    }

    let exit_at = *exits.iter().next().expect("one exit");
    let sources = exiting.iter().map(|&(source, _)| source).collect::<BTreeSet<_>>();
    let predecessors = loops::predecessors(&body.blocks);
    if predecessors[&exit_at] != sources {
        return Ok(body.clone());
    }

    let mut defined: IndexMap<Value, i64> = IndexMap::default();
    for block in body.blocks.iter().filter(|block| loop_.body.contains(&block.at)) {
        let values = block.phis.iter().map(|phi| phi.result).chain(block.ops.iter().flat_map(|op| op.defines.iter().copied()));
        for value in values.filter(|value| !value.flags) {
            defined.insert(value, block.at);
        }
    }
    if defined.is_empty() {
        return Ok(body.clone());
    }

    // Phi inputs are used on their incoming edge.  A phi in the dedicated
    // exit is already the LCSSA boundary, so only downstream phis count here.
    let mut use_sites: IndexMap<Value, BTreeSet<i64>> = IndexMap::default();
    for block in &body.blocks {
        if loop_.body.contains(&block.at) {
            continue;
        }
        for op in &block.ops {
            for value in &op.uses {
                if defined.contains_key(value) {
                    use_sites.entry(*value).or_default().insert(block.at);
                }
            }
        }
        if block.at == exit_at {
            continue;
        }
        for phi in &block.phis {
            for (&predecessor, value) in phi.incoming.iter() {
                if defined.contains_key(value) {
                    use_sites.entry(*value).or_default().insert(predecessor);
                }
            }
        }
    }

    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let empty = BTreeSet::new();
    let crossing = use_sites
        .iter()
        .filter(|(value, sites)| {
            !sites.is_empty()
                && sites.iter().all(|site| dominators.get(site).unwrap_or(&empty).contains(&exit_at))
                && sources.iter().all(|source| dominators.get(source).unwrap_or(&empty).contains(&defined[*value]))
        })
        .map(|(value, _)| *value)
        .collect::<Vec<_>>();
    if crossing.is_empty() {
        return Ok(body.clone());
    }

    let values = ssa::values(body).collect::<Vec<_>>();
    let next_id = values.iter().map(|value| i64::from(value.id)).max().unwrap_or(-1) + 1;
    let mut next_version = crossing
        .iter()
        .map(|one| one.variable)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|variable| {
            let top = values.iter().filter(|one| one.variable == variable).map(|one| one.version).max();
            (variable, top.expect("max() arg is an empty sequence") + 1)
        })
        .collect::<BTreeMap<_, _>>();
    let mut swap: BTreeMap<u32, Value> = BTreeMap::new();
    let mut phis: Vec<Phi> = Vec::new();
    let mut ordered = crossing.clone();
    ordered.sort_by_key(|one| (one.variable, one.version, one.id));
    for (offset, value) in ordered.into_iter().enumerate() {
        // LCSSA closes the live range; it does not invent a new source-level
        // variable.  Keeping the variable identity is what lets recurrence
        // analysis continue through this phi.
        let result = Value {
            id: (next_id + offset as i64) as u32,
            at: exit_at,
            flags: false,
            variable: value.variable,
            version: next_version[&value.variable],
        };
        *next_version.get_mut(&value.variable).expect("present") += 1;
        swap.insert(value.id, result);
        phis.push(Phi { result, incoming: sources.iter().map(|&source| (source, value)).collect() });
    }

    let rewritten = |block: &MirBlock| -> Result<MirBlock, String> {
        if loop_.body.contains(&block.at) {
            return Ok(block.clone());
        }
        let mut existing = block.phis.clone();
        if block.at != exit_at {
            existing = existing
                .into_iter()
                .map(|phi| {
                    let incoming = phi
                        .incoming
                        .iter()
                        .map(|(&predecessor, &value)| {
                            if dominators.get(&predecessor).unwrap_or(&empty).contains(&exit_at) {
                                ssa::provider(value, &swap).map(|one| (predecessor, one))
                            } else {
                                Ok((predecessor, value))
                            }
                        })
                        .collect::<Result<_, _>>()
                        .map_err(|error| error.to_string())?;
                    Ok(Phi { incoming, ..phi })
                })
                .collect::<Result<Vec<_>, String>>()?;
        }
        if block.at == exit_at {
            existing.extend(phis.iter().cloned());
        }
        let ops = if dominators.get(&block.at).unwrap_or(&empty).contains(&exit_at) {
            block
                .ops
                .iter()
                .map(|op| ssa::substituted(op, &swap).map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            block.ops.clone()
        };
        Ok(MirBlock { phis: existing, ops, ..block.clone() })
    };

    let blocks = body.blocks.iter().map(rewritten).collect::<Result<Vec<_>, _>>()?;
    Ok(MirBody { blocks, ..body.clone() })
}

#[cfg(test)]
#[path = "lcssa_tests.rs"]
pub(crate) mod tests;
