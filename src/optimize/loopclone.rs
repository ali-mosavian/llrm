//! Port of `qbopt/optimize/loopclone.py`: clone loop iterations as CFGs,
//! retaining a residual loop and every exit.
//!
//! This is the MIR building block for peeling and bounded full unrolling,
//! not a profitability decision. Loop live-outs must already be in LCSSA.
//! Python's `ValueError`s are the `Err` text.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::loops::{self, Loop};
use crate::analysis::ssa;
use crate::model::mir::{Arg, Held, Kind, MirBlock, MirBody, Phi, Value};
use crate::optimize::edges;

/// Whether conditional cloning cannot create a floating-value join.
///
/// If every floating value is defined and consumed in its own block,
/// lowering can allocate an independent floating region for either arm.
/// Raw raised stack effects still refuse.
pub(crate) fn _block_local_floating(body: &MirBody, originals: &[&MirBlock]) -> bool {
    let inside = originals.iter().map(|block| block.at).collect::<BTreeSet<_>>();
    if originals.iter().any(|block| block.ops.iter().any(|op| op.stack.is_some())) {
        return false;
    }
    let mut owners: BTreeMap<Value, i64> = BTreeMap::new();
    for block in originals {
        for op in block.ops.iter().filter(|op| op.floating.is_some()) {
            for value in &op.defines {
                owners.insert(*value, block.at);
            }
        }
    }
    if owners.is_empty() {
        return true;
    }
    let mut users: BTreeMap<Value, BTreeSet<i64>> = owners.keys().map(|value| (*value, BTreeSet::new())).collect();
    for block in &body.blocks {
        for phi in &block.phis {
            for value in phi.incoming.values() {
                if let Some(found) = users.get_mut(value) {
                    found.insert(block.at);
                }
            }
        }
        for op in &block.ops {
            for value in &op.uses {
                if let Some(found) = users.get_mut(value) {
                    found.insert(block.at);
                }
            }
            if !inside.contains(&block.at) || op.floating.is_none() {
                continue;
            }
            for arg in &op.args {
                if let Arg::Held(held) = arg {
                    if held.width == 10 && owners.get(&held.value) != Some(&block.at) {
                        return false;
                    }
                }
            }
        }
    }
    owners.iter().all(|(value, owner)| users[value] == BTreeSet::from([*owner]))
}

pub(crate) fn peeled(body: &MirBody, loop_: &Loop, count: i64) -> Result<Option<MirBody>, String> {
    if count < 1 {
        return Err("peeling needs a positive iteration count".to_owned());
    }
    let count = count as usize;
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(&body.blocks);
    if loop_.header == body.entry || loop_.latches.len() != 1 {
        return Ok(None);
    }
    let outside = predecessors[&loop_.header].difference(&loop_.body).copied().collect::<Vec<_>>();
    if outside.len() != 1 {
        return Ok(None);
    }
    let entry = outside[0];
    let latch = *loop_.latches.iter().next().expect("one latch");
    if blocks[&entry].succ != [loop_.header] || blocks[&latch].succ != [loop_.header] {
        return Ok(None);
    }
    if loop_.body.iter().any(|at| *at != loop_.header && !predecessors[at].is_subset(&loop_.body)) {
        return Ok(None);
    }
    let originals = body.blocks.iter().filter(|block| loop_.body.contains(&block.at)).collect::<Vec<_>>();
    // A straight-line floating region can be cloned and allocated as one x87
    // sequence.  An internal conditional needs a stronger proof: folding a
    // cloned selector may delete a different arm in every copy.
    if originals.iter().any(|block| block.ops.iter().any(|op| op.floating.is_some()))
        && originals.iter().any(|block| block.at != loop_.header && block.succ.len() > 1)
        && !_block_local_floating(body, &originals)
    {
        return Ok(None);
    }
    if originals.iter().any(|block| {
        block.succ.len() > 1
            && match block.ops.last() {
                None => true,
                Some(last) => {
                    let branch = last.kind == Kind::Branch
                        && block.succ.len() == 2
                        && last.target.is_some_and(|target| block.succ.contains(&target));
                    let switch = last.kind == Kind::Switch
                        && last.target.is_some_and(|target| {
                            block.succ.iter().copied().collect::<BTreeSet<_>>()
                                == std::iter::once(target)
                                    .chain(last.cases.iter().map(|&(_, case)| case))
                                    .collect::<BTreeSet<_>>()
                        });
                    !(branch || switch)
                }
            }
    }) {
        return Ok(None);
    }
    let defined = originals
        .iter()
        .flat_map(|block| {
            block.phis.iter().map(|phi| phi.result).chain(block.ops.iter().flat_map(|op| op.defines.iter().copied()))
        })
        .collect::<BTreeSet<_>>();
    for block in &body.blocks {
        if loop_.body.contains(&block.at) {
            continue;
        }
        if block.ops.iter().any(|op| op.uses.iter().any(|value| defined.contains(value))) {
            return Ok(None);
        }
        if block.phis.iter().any(|phi| {
            phi.incoming.iter().any(|(source, value)| defined.contains(value) && !loop_.body.contains(source))
        }) {
            return Ok(None);
        }
    }

    let all_values = ssa::values(body).collect::<Vec<_>>();
    let mut next_id = all_values.iter().map(|value| value.id).max().unwrap_or(0) + 1;
    let mut next_variable = all_values.iter().map(|value| value.variable).max().unwrap_or(0) + 1;
    let mut next_label = edges::fresh(body);
    let mut labels: Vec<BTreeMap<i64, i64>> = Vec::new();
    let mut copies: Vec<BTreeMap<u32, Value>> = Vec::new();
    let mut ordered = defined.iter().copied().collect::<Vec<_>>();
    ordered.sort_by_key(|value| value.id);
    for _ in 0..count {
        labels.push(originals.iter().enumerate().map(|(index, block)| (block.at, next_label + index as i64)).collect());
        next_label += originals.len() as i64;
        copies.push(BTreeMap::new());
        for value in &ordered {
            let copy = Value { id: next_id, variable: next_variable, version: 1, ..*value };
            copies.last_mut().expect("pushed").insert(value.id, copy);
            next_id += 1;
            next_variable += 1;
        }
    }

    let value = |original: Value, iteration: usize| copies[iteration].get(&original.id).copied().unwrap_or(original);
    let destination = |at: i64, source: i64, iteration: usize| {
        if source == latch && at == loop_.header {
            return if iteration + 1 < count { labels[iteration + 1][&at] } else { at };
        }
        labels[iteration].get(&at).copied().unwrap_or(at)
    };

    let mut cloned = Vec::new();
    for iteration in 0..count {
        for block in &originals {
            let mut phis = Vec::new();
            for phi in &block.phis {
                let incoming = if block.at == loop_.header {
                    if iteration == 0 {
                        [(entry, *phi.incoming.get(&entry).expect("KeyError"))].into_iter().collect()
                    } else {
                        let from = *phi.incoming.get(&latch).expect("KeyError");
                        [(labels[iteration - 1][&latch], value(from, iteration - 1))].into_iter().collect()
                    }
                } else {
                    phi.incoming
                        .iter()
                        .map(|(source, &incoming)| (labels[iteration][source], value(incoming, iteration)))
                        .collect()
                };
                phis.push(Phi { result: value(phi.result, iteration), incoming });
            }
            let mut ops = Vec::new();
            for op in &block.ops {
                let mut read = ssa::substituted(op, &copies[iteration]).map_err(|error| error.to_string())?;
                read.defines = op.defines.iter().map(|&result| value(result, iteration)).collect();
                read.results = read
                    .results
                    .iter()
                    .map(|result| match result {
                        Arg::Held(held) => Arg::Held(Held { value: value(held.value, iteration), width: held.width }),
                        _ => result.clone(),
                    })
                    .collect();
                read.target = op.target.map(|target| destination(target, block.at, iteration));
                read.cases =
                    op.cases.iter().map(|&(number, target)| (number, destination(target, block.at, iteration))).collect();
                read.absorbed = Vec::new();
                read.raised = None;
                read.symbol = Some(op.symbol != Some(false));
                ops.push(read);
            }
            cloned.push(MirBlock::new(
                labels[iteration][&block.at],
                phis,
                ops,
                block.succ.iter().map(|&at| destination(at, block.at, iteration)).collect(),
            ));
        }
    }

    let mut changed = Vec::new();
    for block in &body.blocks {
        let mut block = block.clone();
        if block.at == entry {
            let first = labels[0][&loop_.header];
            block.succ = vec![first];
            for op in &mut block.ops {
                if op.target == Some(loop_.header) {
                    op.target = Some(first);
                }
            }
        }
        let mut phis = Vec::new();
        for phi in &block.phis {
            let mut incoming = phi.incoming.clone();
            if block.at == loop_.header {
                incoming.remove(&entry).expect("KeyError");
                let from = *phi.incoming.get(&latch).expect("KeyError");
                incoming.insert(labels[count - 1][&latch], value(from, count - 1));
            } else if !loop_.body.contains(&block.at) {
                for (source, &original) in phi.incoming.iter() {
                    if loop_.body.contains(source) {
                        for (iteration, label) in labels.iter().enumerate() {
                            incoming.insert(label[source], value(original, iteration));
                        }
                    }
                }
            }
            phis.push(Phi { incoming, ..phi.clone() });
        }
        block.phis = phis;
        changed.push(block);
    }

    let (pointer_values, pointer_seeds) = ssa::cloned_pointer_metadata(body, copies.iter());
    let integer_ranges = ssa::cloned_integer_ranges(body, copies.iter());
    changed.extend(cloned);
    Ok(Some(MirBody { blocks: changed, cloned: true, pointer_values, pointer_seeds, integer_ranges, ..body.clone() }))
}

#[cfg(test)]
#[path = "loopclone_tests.rs"]
mod tests;
