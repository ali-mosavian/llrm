//! Port of `qbopt/optimize/lcssamerges.py`.

// ---- early port (agent E) ----
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::loops::{self, Loop};
use crate::analysis::ssa;
use crate::model::mir::{MirBody, OrderedMap, Phi, Value};

pub(crate) fn closed(body: &MirBody, loop_: &Loop) -> Result<MirBody, String> {
    let mut body = body.clone();
    let predecessors = loops::predecessors(&body.blocks);
    let dominators = loops::dominators(&body.blocks, body.entry);
    let exits = body
        .blocks
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .flat_map(|block| block.succ.iter().copied())
        .filter(|at| !loop_.body.contains(at))
        .collect::<BTreeSet<i64>>();
    if exits.iter().any(|at| {
        predecessors
            .get(at)
            .is_none_or(|parents| parents.is_empty() || !parents.is_subset(&loop_.body))
    }) {
        return Ok(body);
    }
    let mut definitions = BTreeMap::<Value, i64>::new();
    for block in &body.blocks {
        if !loop_.body.contains(&block.at) {
            continue;
        }
        for value in block
            .phis
            .iter()
            .map(|phi| phi.result)
            .chain(block.ops.iter().flat_map(|op| op.defines.iter().copied()))
        {
            if !value.flags {
                definitions.insert(value, block.at);
            }
        }
    }
    let mut sites = BTreeMap::<Value, BTreeSet<i64>>::new();
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
            for (parent, value) in phi.incoming.iter() {
                if !loop_.body.contains(parent) && definitions.contains_key(value) {
                    sites.entry(*value).or_default().insert(*parent);
                }
            }
        }
    }
    let empty = BTreeSet::new();
    let mut ordered = sites.keys().copied().collect::<Vec<_>>();
    ordered.sort_by_key(|value| value.id);
    for value in ordered {
        let available = exits
            .iter()
            .copied()
            .filter(|at| {
                predecessors[at]
                    .iter()
                    .all(|parent| dominators.get(parent).unwrap_or(&empty).contains(&definitions[&value]))
            })
            .collect::<BTreeSet<i64>>();
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
            let reaching = available.intersection(&needed).copied().collect();
            body = _merged(&body, value, &needed, &reaching, &predecessors)?;
        }
    }
    Ok(body)
}

fn _merged(
    body: &MirBody,
    value: Value,
    needed: &BTreeSet<i64>,
    exits: &BTreeSet<i64>,
    predecessors: &BTreeMap<i64, BTreeSet<i64>>,
) -> Result<MirBody, String> {
    let mut reaching = needed
        .iter()
        .map(|at| (*at, if exits.contains(at) { BTreeSet::from([*at]) } else { BTreeSet::new() }))
        .collect::<BTreeMap<i64, BTreeSet<i64>>>();
    loop {
        let changed = needed
            .iter()
            .map(|at| {
                let found = if exits.contains(at) {
                    reaching[at].clone()
                } else {
                    predecessors[at]
                        .iter()
                        .flat_map(|parent| reaching[parent].iter().copied())
                        .collect()
                };
                (*at, found)
            })
            .collect::<BTreeMap<i64, BTreeSet<i64>>>();
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
    let version = values
        .iter()
        .filter(|one| one.variable == value.variable)
        .map(|one| one.version)
        .max()
        .unwrap_or(0)
        + 1;
    let mut joins = exits.clone();
    joins.extend(
        needed
            .iter()
            .copied()
            .filter(|at| reaching[at].len() > 1 && predecessors[at].len() > 1),
    );
    let mut replacements = joins
        .iter()
        .enumerate()
        .map(|(offset, at)| {
            (
                *at,
                Value {
                    id: u32::try_from(serial + offset as i64).expect("value id fits"),
                    at: *at,
                    flags: false,
                    variable: value.variable,
                    version: version + offset as u32,
                },
            )
        })
        .collect::<BTreeMap<i64, Value>>();
    let mut pending = needed.difference(&joins).copied().collect::<BTreeSet<i64>>();
    while !pending.is_empty() {
        let mut changed = BTreeSet::new();
        for at in &pending {
            if reaching[at].len() == 1 {
                let only = *reaching[at].first().expect("one source");
                replacements.insert(*at, replacements[&only]);
            } else {
                let [parent] = predecessors[at].iter().copied().collect::<Vec<_>>()[..] else {
                    panic!("not exactly one value to unpack");
                };
                let Some(found) = replacements.get(&parent).copied() else {
                    continue;
                };
                replacements.insert(*at, found);
            }
            changed.insert(*at);
        }
        if changed.is_empty() {
            return Ok(body.clone());
        }
        pending = pending.difference(&changed).copied().collect();
    }
    let phis = joins
        .iter()
        .map(|at| {
            let mut phi = Phi::new(replacements[at]);
            phi.incoming = predecessors[at]
                .iter()
                .map(|parent| (*parent, if exits.contains(at) { value } else { replacements[parent] }))
                .collect::<OrderedMap<_, _>>();
            (*at, phi)
        })
        .collect::<BTreeMap<i64, Phi>>();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut block = block.clone();
        for phi in &mut block.phis {
            phi.incoming = phi
                .incoming
                .iter()
                .map(|(parent, incoming)| {
                    let incoming = match replacements.get(parent) {
                        Some(replacement) if *incoming == value => *replacement,
                        _ => *incoming,
                    };
                    (*parent, incoming)
                })
                .collect();
        }
        if let Some(phi) = phis.get(&block.at) {
            block.phis.push(phi.clone());
        }
        if let Some(replacement) = replacements.get(&block.at) {
            let swap = BTreeMap::from([(value.id, *replacement)]);
            block.ops = block
                .ops
                .iter()
                .map(|op| ssa::substituted(op, &swap).map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
        }
        blocks.push(block);
    }
    Ok(MirBody { blocks, ..body.clone() })
}
