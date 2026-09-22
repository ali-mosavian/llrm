//! Port of `qbopt/optimize/lcssamerges.py`.
//!
//! Python's `ValueError` from `ssa.substituted` is the `Err` text.

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use crate::analysis::loops::{self, Loop};
use crate::analysis::ssa;
use crate::model::mir::{MirBlock, MirBody, Phi, Value};

pub(crate) fn closed(body: &MirBody, loop_: &Loop) -> Result<MirBody, String> {
    let mut body = body.clone();
    let predecessors = loops::predecessors(&body.blocks);
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let exits = body
        .blocks
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .flat_map(|block| block.succ.iter().copied())
        .filter(|at| !loop_.body.contains(at))
        .collect::<BTreeSet<_>>();
    if exits.iter().any(|at| {
        predecessors.get(at).is_none_or(|parents| parents.is_empty() || !parents.is_subset(&loop_.body))
    }) {
        return Ok(body);
    }
    let mut definitions: IndexMap<Value, i64> = IndexMap::new();
    for block in body.blocks.iter().filter(|block| loop_.body.contains(&block.at)) {
        let values = block.phis.iter().map(|phi| phi.result).chain(block.ops.iter().flat_map(|op| op.defines.iter().copied()));
        for value in values.filter(|value| !value.flags) {
            definitions.insert(value, block.at);
        }
    }
    let mut sites: IndexMap<Value, BTreeSet<i64>> = IndexMap::new();
    for block in &body.blocks {
        if loop_.body.contains(&block.at) {
            continue;
        }
        for op in &block.ops {
            for value in &op.uses {
                if definitions.contains_key(value) {
                    sites.entry(*value).or_default().insert(block.at);
                }
            }
        }
        for phi in &block.phis {
            for (&parent, value) in phi.incoming.iter() {
                if !loop_.body.contains(&parent) && definitions.contains_key(value) {
                    sites.entry(*value).or_default().insert(parent);
                }
            }
        }
    }
    let mut ordered = sites.keys().copied().collect::<Vec<_>>();
    ordered.sort_by_key(|value| value.id);
    let empty = BTreeSet::new();
    for value in ordered {
        let available = exits
            .iter()
            .copied()
            .filter(|at| {
                predecessors[at]
                    .iter()
                    .all(|parent| dominators.get(parent).unwrap_or(&empty).contains(&definitions[&value]))
            })
            .collect::<BTreeSet<_>>();
        let mut needed = BTreeSet::new();
        let mut pending = sites[&value].iter().copied().collect::<Vec<_>>();
        let mut broke = false;
        while let Some(at) = pending.pop() {
            if needed.contains(&at) {
                continue;
            }
            if loop_.body.contains(&at) || at == body.entry || predecessors.get(&at).is_none_or(BTreeSet::is_empty) {
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
            body = _merged(&body, value, &needed, &reached, &predecessors)?;
        }
    }
    Ok(body)
}

pub(crate) fn _merged(
    body: &MirBody,
    value: Value,
    needed: &BTreeSet<i64>,
    exits: &BTreeSet<i64>,
    predecessors: &BTreeMap<i64, BTreeSet<i64>>,
) -> Result<MirBody, String> {
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
        return Ok(body.clone());
    }
    let values = ssa::values(body).collect::<Vec<_>>();
    let serial = values.iter().map(|one| i64::from(one.id)).max().unwrap_or(-1) + 1;
    let version = values.iter().filter(|one| one.variable == value.variable).map(|one| one.version).max().unwrap_or(0) + 1;
    let mut joins = exits.clone();
    joins.extend(needed.iter().copied().filter(|at| reaching[at].len() > 1 && predecessors[at].len() > 1));
    let mut replacements = joins
        .iter()
        .enumerate()
        .map(|(offset, &at)| {
            let fresh = Value {
                id: (serial + offset as i64) as u32,
                at,
                flags: false,
                variable: value.variable,
                version: version + offset as u32,
            };
            (at, fresh)
        })
        .collect::<BTreeMap<_, _>>();
    let mut pending = needed.difference(&joins).copied().collect::<BTreeSet<_>>();
    while !pending.is_empty() {
        let mut changed = BTreeSet::new();
        for &at in &pending {
            if reaching[&at].len() == 1 {
                let only = *reaching[&at].iter().next().expect("one");
                replacements.insert(at, replacements[&only]);
            } else {
                let parents = &predecessors[&at];
                if parents.len() > 1 {
                    return Err("too many values to unpack (expected 1)".to_owned());
                }
                if parents.is_empty() {
                    return Err("not enough values to unpack (expected 1, got 0)".to_owned());
                }
                let parent = *parents.iter().next().expect("one");
                let Some(&found) = replacements.get(&parent) else {
                    continue;
                };
                replacements.insert(at, found);
            }
            changed.insert(at);
        }
        if changed.is_empty() {
            return Ok(body.clone());
        }
        pending = pending.difference(&changed).copied().collect();
    }
    let phis = joins
        .iter()
        .map(|&at| {
            let incoming = predecessors[&at]
                .iter()
                .map(|&parent| (parent, if exits.contains(&at) { value } else { replacements[&parent] }))
                .collect();
            (at, Phi { result: replacements[&at], incoming })
        })
        .collect::<BTreeMap<_, _>>();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut phis_here = block
            .phis
            .iter()
            .map(|phi| Phi {
                incoming: phi
                    .incoming
                    .iter()
                    .map(|(&parent, &incoming)| match replacements.get(&parent) {
                        Some(&replacement) if incoming == value => (parent, replacement),
                        _ => (parent, incoming),
                    })
                    .collect(),
                ..phi.clone()
            })
            .collect::<Vec<_>>();
        if let Some(phi) = phis.get(&block.at) {
            phis_here.push(phi.clone());
        }
        let ops = match replacements.get(&block.at) {
            Some(&replacement) => {
                let swap = BTreeMap::from([(value.id, replacement)]);
                block
                    .ops
                    .iter()
                    .map(|op| ssa::substituted(op, &swap).map_err(|error| error.to_string()))
                    .collect::<Result<Vec<_>, _>>()?
            }
            None => block.ops.clone(),
        };
        blocks.push(MirBlock { phis: phis_here, ops, ..block.clone() });
    }
    Ok(MirBody { blocks, ..body.clone() })
}

#[cfg(test)]
#[path = "lcssamerges_tests.rs"]
mod tests;
