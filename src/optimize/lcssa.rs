//! Port of `qbopt/optimize/lcssa.py`.

// ---- early port (agent E) ----
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use crate::analysis::loops::{self, Loop};
use crate::analysis::ssa;
use crate::model::mir::{MirBlock, MirBody, OrderedMap, Phi, Value};
use crate::model::passes::MIRTransform;
use crate::optimize::lcssamerges;

pub struct LoopClosedSSA;

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

fn _closed_loop(body: &MirBody, loop_: &Loop) -> Result<MirBody, String> {
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<i64, &MirBlock>>();
    let exiting = body
        .blocks
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .flat_map(|block| block.succ.iter().map(move |successor| (block.at, *successor)))
        .filter(|(_, successor)| blocks.contains_key(successor) && !loop_.body.contains(successor))
        .collect::<Vec<_>>();
    let exits = exiting.iter().map(|(_, target)| *target).collect::<BTreeSet<i64>>();
    if exits.len() > 1 {
        return lcssamerges::closed(body, loop_);
    }
    let Some(&exit_at) = exits.first() else {
        return Ok(body.clone());
    };
    let sources = exiting.iter().map(|(source, _)| *source).collect::<BTreeSet<i64>>();
    let predecessors = loops::predecessors(&body.blocks);
    if predecessors[&exit_at] != sources {
        return Ok(body.clone());
    }

    let mut defined = BTreeMap::<Value, i64>::new();
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
                defined.insert(value, block.at);
            }
        }
    }
    if defined.is_empty() {
        return Ok(body.clone());
    }

    // Phi inputs are used on their incoming edge.  A phi in the dedicated
    // exit is already the LCSSA boundary, so only downstream phis count here.
    let mut use_sites = IndexMap::<Value, BTreeSet<i64>>::new();
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
            for (predecessor, value) in phi.incoming.iter() {
                if defined.contains_key(value) {
                    use_sites.entry(*value).or_default().insert(*predecessor);
                }
            }
        }
    }

    let dominators = loops::dominators(&body.blocks, body.entry);
    let empty = BTreeSet::new();
    let dominated = |at: &i64| dominators.get(at).unwrap_or(&empty);
    let crossing = use_sites
        .iter()
        .filter(|(value, sites)| {
            !sites.is_empty()
                && sites.iter().all(|site| dominated(site).contains(&exit_at))
                && sources.iter().all(|source| dominated(source).contains(&defined[value]))
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
        .collect::<BTreeSet<u32>>()
        .into_iter()
        .map(|variable| {
            let top = values
                .iter()
                .filter(|one| one.variable == variable)
                .map(|one| one.version)
                .max()
                .expect("max() arg is an empty sequence");
            (variable, top + 1)
        })
        .collect::<BTreeMap<u32, u32>>();
    let mut swap = BTreeMap::<u32, Value>::new();
    let mut phis = Vec::<Phi>::new();
    let mut ordered = crossing.clone();
    ordered.sort_by_key(|one| (one.variable, one.version, one.id));
    for (offset, value) in ordered.iter().enumerate() {
        // LCSSA closes the live range; it does not invent a new source-level
        // variable.  Keeping the variable identity is what lets recurrence
        // analysis continue through this phi.
        let result = Value {
            id: u32::try_from(next_id + offset as i64).expect("value id fits"),
            at: exit_at,
            flags: false,
            variable: value.variable,
            version: next_version[&value.variable],
        };
        *next_version.get_mut(&value.variable).expect("counted") += 1;
        swap.insert(value.id, result);
        let mut phi = Phi::new(result);
        phi.incoming = sources.iter().map(|source| (*source, *value)).collect::<OrderedMap<_, _>>();
        phis.push(phi);
    }

    let substitution = |error: ssa::SubstitutionError| error.to_string();
    let mut rewritten = Vec::new();
    for block in &body.blocks {
        if loop_.body.contains(&block.at) {
            rewritten.push(block.clone());
            continue;
        }
        let mut existing = block.phis.clone();
        if block.at != exit_at {
            for phi in &mut existing {
                let mut incoming = OrderedMap::new();
                for (predecessor, value) in phi.incoming.iter() {
                    let value = if dominated(predecessor).contains(&exit_at) {
                        ssa::provider(*value, &swap).map_err(substitution)?
                    } else {
                        *value
                    };
                    incoming.insert(*predecessor, value);
                }
                phi.incoming = incoming;
            }
        }
        if block.at == exit_at {
            existing.extend(phis.iter().cloned());
        }
        let ops = if dominated(&block.at).contains(&exit_at) {
            block
                .ops
                .iter()
                .map(|op| ssa::substituted(op, &swap).map_err(substitution))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            block.ops.clone()
        };
        let mut block = block.clone();
        block.phis = existing;
        block.ops = ops;
        rewritten.push(block);
    }
    Ok(MirBody {
        blocks: rewritten,
        ..body.clone()
    })
}
