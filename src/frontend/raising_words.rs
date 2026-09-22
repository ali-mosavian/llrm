//! Port of `qbopt/frontend/raising_words.py`.
//!
//! Normalize unobserved upper-word preservation at the raise boundary.

use std::collections::BTreeSet;

use crate::model::mir::{self, Arg, Held, Kind, Op, OrderedMap, RaisedBody, Value};
use crate::support::hash::{HashMap, IndexMap, IndexSet};

pub fn carried(body: RaisedBody) -> RaisedBody {
    let mut parents: IndexMap<Value, BTreeSet<Value>> = IndexMap::default();
    for block in &body.blocks {
        for phi in &block.phis {
            parents.insert(phi.result, phi.incoming.values().copied().collect());
        }
        for op in &block.ops {
            for (source, result) in op.merges.iter() {
                if op.results.contains(&Arg::Held(Held { value: *result, width: 2 })) {
                    parents.insert(*result, BTreeSet::from([*source]));
                }
            }
        }
    }
    let sources: BTreeSet<Value> = parents.values().flatten().copied().collect();
    let mut roots: HashMap<Value, BTreeSet<Value>> = sources
        .iter()
        .chain(parents.keys())
        .map(|value| (*value, if parents.contains_key(value) { BTreeSet::new() } else { BTreeSet::from([*value]) }))
        .collect();
    loop {
        let mut changed = false;
        for (value, incoming) in &parents {
            let extended: BTreeSet<Value> = incoming.iter().flat_map(|source| roots[source].iter().copied()).collect();
            if extended != roots[value] {
                roots.insert(*value, extended);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let canonical: HashMap<Value, Value> = roots
        .iter()
        .filter(|(_, found)| found.len() == 1)
        .map(|(value, found)| (*value, *found.iter().next().expect("one root")))
        .collect();

    let operation = |op: &Op| -> Op {
        // A merge carries only upper bits; ordinary operands must retain
        // their own values even when they also supply those bits.
        if op.merges.is_empty() {
            return op.clone();
        }
        let merges: OrderedMap<Value, Value> = op
            .merges
            .iter()
            .map(|(source, result)| (canonical.get(source).copied().unwrap_or(*source), *result))
            .collect();
        if merges.len() != op.merges.len() || merges == op.merges {
            return op.clone();
        }
        let mut explicit: BTreeSet<Value> = op
            .args
            .iter()
            .filter_map(|arg| match arg {
                Arg::Held(held) => Some(held.value),
                _ => None,
            })
            .collect();
        explicit.extend(op.loads.iter().chain(&op.stores).flat_map(|r#ref| [r#ref.base, r#ref.segment]).flatten());
        let uses: IndexSet<Value> = op
            .uses
            .iter()
            .copied()
            .filter(|value| !op.merges.contains_key(value) || explicit.contains(value))
            .chain(merges.keys().copied())
            .collect();
        let mut made = op.clone();
        made.merges = merges;
        made.uses = uses.into_iter().collect();
        made
    };

    let blocks = body.blocks.iter().map(|block| block.with_ops(block.ops.iter().map(operation).collect())).collect();
    body.with_blocks(blocks)
}

pub fn scalar(body: RaisedBody) -> RaisedBody {
    let mut required = leaving(&body);
    let mut carrying: Vec<(Value, Value)> = Vec::new();
    // Python's `id(op)`: the operation's (block, index) position.
    let mut candidates: HashMap<(usize, usize), HashMap<Value, u32>> = HashMap::default();
    for (at, block) in body.blocks.iter().enumerate() {
        carrying.extend(block.phis.iter().flat_map(|phi| phi.incoming.values().map(|value| (phi.result, *value))));
        for (index, op) in block.ops.iter().enumerate() {
            carrying.extend(op.merges.iter().map(|(source, result)| (*result, *source)));
            let mut widths: HashMap<Value, u32> = HashMap::default();
            for arg in &op.args {
                if let Arg::Held(held) = arg {
                    let width = widths.get(&held.value).copied().unwrap_or(0).max(held.width);
                    widths.insert(held.value, width);
                }
            }
            for r#ref in op.loads.iter().chain(&op.stores) {
                if let Some(base) = r#ref.base {
                    let width = widths.get(&base).copied().unwrap_or(0).max(r#ref.base_width);
                    widths.insert(base, width);
                }
                if let Some(segment) = r#ref.segment {
                    widths.insert(segment, 4);
                }
            }
            let narrow = matches!(op.kind, Kind::Load | Kind::Copy)
                && !op.barrier()
                && op.results.len() == 1
                && matches!(&op.results[0], Arg::Held(held) if held.width == 2
                    && op.merges.values().all(|value| *value == held.value));
            for value in &op.uses {
                if value.flags {
                    continue;
                }
                if op.barrier()
                    || op.kind == Kind::Opaque
                    || widths.get(value).copied().unwrap_or(0) >= 4
                    || (!widths.contains_key(value) && !(narrow && op.merges.contains_key(value)))
                {
                    required.insert(*value);
                }
            }
            if narrow {
                candidates.insert((at, index), widths);
            }
        }
    }
    loop {
        let mut extended = required.clone();
        extended.extend(carrying.iter().filter(|(result, _)| required.contains(result)).map(|(_, source)| *source));
        if extended == required {
            break;
        }
        required = extended;
    }
    let mut blocks = Vec::new();
    for (at, block) in body.blocks.iter().enumerate() {
        let mut ops = Vec::new();
        for (index, op) in block.ops.iter().enumerate() {
            let mut op = op.clone();
            if let Some(widths) = candidates.get(&(at, index)) {
                let removed: BTreeSet<Value> = op
                    .merges
                    .iter()
                    .filter(|(_, result)| !required.contains(result))
                    .map(|(source, _)| *source)
                    .collect();
                if !removed.is_empty() {
                    op.merges = op
                        .merges
                        .iter()
                        .filter(|(source, _)| !removed.contains(source))
                        .map(|(source, result)| (*source, *result))
                        .collect();
                    op.uses.retain(|value| !removed.contains(value) || widths.contains_key(value));
                }
            }
            ops.push(op);
        }
        blocks.push(block.with_ops(ops));
    }
    carried(body.with_blocks(blocks))
}

/// The value each register holds where control leaves the body.
///
/// What the caller reads is not a fact this body holds, so everything that
/// reaches an exit counts as read. This is the one place the liveness looks
/// at `origin`: "what the caller sees" is a statement about registers.
pub fn leaving(body: &RaisedBody) -> BTreeSet<Value> {
    mir::live_outs(body).into_values().flatten().collect()
}

#[cfg(test)]
#[path = "raising_words_tests.rs"]
mod tests;
