//! Discard exact, unused conversions while retaining floating observation points.
//!
//! Port of `qbopt/optimize/floatfold.py`.

// Its callers live in transform.py, not yet ported.
#![allow(dead_code)]

use std::collections::BTreeMap;

use indexmap::IndexMap;

use crate::analysis::consts::Known;
use crate::analysis::floatfacts::{self, Finite};
use crate::model::floating::{Exceptions, Format};
use crate::model::mir::{Arg, Cell, Const, Kind, MirBody, Op, OrderedMap, Value};

fn _checked(op: &Op) -> Op {
    let kind = if op
        .floating
        .as_ref()
        .is_some_and(|floating| floating.exceptions == Exceptions::Deferred)
    {
        Kind::Nothing
    } else {
        Kind::Fcheck
    };
    let mut op = op.clone();
    op.kind = kind;
    op.name = String::new();
    op.args = Vec::new();
    op.results = Vec::new();
    op.uses = Vec::new();
    op.defines = Vec::new();
    op.loads = Vec::new();
    op.stores = Vec::new();
    op.merges = OrderedMap::new();
    op.source_backed = false;
    op.raised = None;
    op.floating = None;
    op.stack = None;
    op.symbol = Some(false);
    op
}

/// Python's `Counter`: a missing value reads zero.
fn _reads(body: &MirBody) -> BTreeMap<Value, usize> {
    let mut reads = BTreeMap::new();
    for value in body
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter())
        .flat_map(|op| op.uses.iter())
    {
        *reads.entry(*value).or_insert(0) += 1;
    }
    for value in body
        .blocks
        .iter()
        .flat_map(|block| block.phis.iter())
        .flat_map(|phi| phi.incoming.values())
    {
        *reads.entry(*value).or_insert(0) += 1;
    }
    reads
}

fn count(reads: &BTreeMap<Value, usize>, value: &Value) -> usize {
    reads.get(value).copied().unwrap_or(0)
}

fn _observed_after(op: &Op, observed: bool) -> bool {
    if op.barrier()
        || op.floating.is_some()
        || op.stack.is_some()
        || op
            .args
            .iter()
            .chain(op.results.iter())
            .any(|arg| matches!(arg, Arg::Held(held) if held.width == 10))
    {
        return false;
    }
    if op.kind == Kind::Fcheck {
        return true;
    }
    let transparent = [
        Kind::Nothing,
        Kind::Copy,
        Kind::Load,
        Kind::Store,
        Kind::Arg,
        Kind::Add,
        Kind::Sub,
        Kind::Increment,
        Kind::Branch,
        Kind::Jump,
        Kind::Lt,
        Kind::Le,
        Kind::Gt,
        Kind::Ge,
        Kind::Eq,
        Kind::Ne,
    ];
    observed && transparent.contains(&op.kind)
}

/// A completed observation stays satisfied until floating or unknown work.
pub fn checks(body: &MirBody) -> MirBody {
    let mut predecessors = body
        .blocks
        .iter()
        .map(|block| (block.at, Vec::new()))
        .collect::<IndexMap<i64, Vec<i64>>>();
    for block in &body.blocks {
        for successor in &block.succ {
            if let Some(parents) = predecessors.get_mut(successor) {
                parents.push(block.at);
            }
        }
    }
    let mut entries = predecessors
        .keys()
        .map(|at| (*at, false))
        .collect::<IndexMap<i64, bool>>();
    let mut exits = entries.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for block in &body.blocks {
            let parents = &predecessors[&block.at];
            let mut observed = !parents.is_empty()
                && block.at != body.entry
                && parents.iter().all(|at| exits[at]);
            entries.insert(block.at, observed);
            for op in &block.ops {
                observed = _observed_after(op, observed);
            }
            if exits[&block.at] != observed {
                exits.insert(block.at, observed);
                changed = true;
            }
        }
    }
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut observed = entries[&block.at];
        let mut ops = Vec::new();
        for op in &block.ops {
            let after = _observed_after(op, observed);
            let mut op = op.clone();
            if op.kind == Kind::Fcheck && observed && after {
                op.kind = Kind::Nothing;
                op.source_backed = false;
                op.raised = None;
                op.symbol = Some(false);
            }
            observed = after;
            ops.push(op);
        }
        let mut block = block.clone();
        block.ops = ops;
        blocks.push(block);
    }
    MirBody {
        blocks,
        ..body.clone()
    }
}

/// Write exact storage bits and retain checks for the now-unused computation.
pub fn stored(body: &MirBody, facts: &IndexMap<Value, Finite>) -> MirBody {
    if facts.is_empty() {
        return body.clone();
    }
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            if let (Kind::Fstore, Some(floating), [Arg::Held(held)]) =
                (op.kind, op.floating.as_ref(), op.args.as_slice())
            {
                if matches!(floating.result, Format::Binary32 | Format::Binary64)
                    && !op.barrier()
                    && op.stores.len() == 1
                    && op.defines.is_empty()
                    && facts.contains_key(&held.value)
                {
                    let reference = &op.stores[0];
                    let value =
                        floatfacts::evaluated(op.kind, floating, &[facts[&held.value].clone()]);
                    let format = floating.result;
                    let width = if format == Format::Binary32 { 4 } else { 8 };
                    let bits = value
                        .as_ref()
                        .and_then(|value| floatfacts::encoded(value, format));
                    if let Some(bits) = bits {
                        if reference.width == width
                            && reference.base.is_none()
                            && reference.segment.is_none()
                            && reference.addr.is_some()
                        {
                            ops.push(_checked(op));
                            let mut store = op.clone();
                            store.kind = Kind::Store;
                            store.name = String::new();
                            store.args = vec![Arg::Const(Const::new(bits, width))];
                            store.results = vec![Arg::Cell(Cell {
                                r#ref: reference.clone(),
                            })];
                            store.uses = Vec::new();
                            store.loads = Vec::new();
                            store.merges = OrderedMap::new();
                            store.source_backed = false;
                            store.raised = None;
                            store.floating = None;
                            store.floating_origin = None;
                            store.stack = None;
                            store.absorbed = Vec::new();
                            store.symbol = Some(true);
                            ops.push(store);
                            continue;
                        }
                    }
                }
            }
            ops.push(op.clone());
        }
        let mut block = block.clone();
        block.ops = ops;
        blocks.push(block);
    }
    _dead_values(
        MirBody {
            blocks,
            ..body.clone()
        },
        facts,
    )
}

fn _dead_values(mut changed: MirBody, facts: &IndexMap<Value, Finite>) -> MirBody {
    loop {
        let reads = _reads(&changed);
        let mut removed = false;
        let mut blocks = Vec::new();
        for block in &changed.blocks {
            let mut ops = Vec::new();
            for op in &block.ops {
                let mut op = op.clone();
                if op.floating.is_some()
                    && op.stores.is_empty()
                    && !op.barrier()
                    && op.results.len() == 1
                    && matches!(&op.results[0], Arg::Held(held) if held.width == 10 && facts.contains_key(&held.value))
                    && !op.defines.iter().any(|value| count(&reads, value) != 0)
                {
                    op = _checked(&op);
                    removed = true;
                }
                ops.push(op);
            }
            let mut block = block.clone();
            block.ops = ops;
            blocks.push(block);
        }
        changed = MirBody {
            blocks,
            ..changed
        };
        if !removed {
            return changed;
        }
    }
}

pub fn discarded(body: &MirBody, converted: &IndexMap<Value, Known>) -> MirBody {
    if converted.is_empty() {
        return body.clone();
    }
    let reads = _reads(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = block.ops.clone();
        for index in 0..ops.len() {
            let conversion = &ops[index];
            if conversion.kind != Kind::Fstore
                || !conversion.stores.is_empty()
                || conversion.barrier()
                || conversion.results.len() != 1
                || conversion.args.len() != 1
            {
                continue;
            }
            let (source, result) = (&conversion.args[0], &conversion.results[0]);
            let Arg::Held(result) = result else {
                continue;
            };
            if !converted.contains_key(&result.value) || count(&reads, &result.value) != 0 {
                continue;
            }
            let Arg::Held(source) = source else {
                continue;
            };
            if source.width != 10 || count(&reads, &source.value) != 1 {
                continue;
            }
            let (source, result) = (*source, *result);
            if conversion
                .defines
                .iter()
                .any(|value| *value != result.value && count(&reads, value) != 0)
            {
                continue;
            }
            let checked = _checked(conversion);
            ops[index] = checked;
            let load = if index != 0 { Some(&ops[index - 1]) } else { None };
            if let Some(load) = load {
                if load.kind == Kind::Fload
                    && !load.barrier()
                    && load.stores.is_empty()
                    && load.results == [Arg::Held(source)]
                    && load.args.len() == 1
                    && matches!(load.args[0], Arg::Cell(_))
                    && !load
                        .defines
                        .iter()
                        .any(|value| *value != source.value && count(&reads, value) != 0)
                {
                    let checked = _checked(load);
                    ops[index - 1] = checked;
                }
            }
        }
        let mut block = block.clone();
        block.ops = ops;
        blocks.push(block);
    }
    MirBody {
        blocks,
        ..body.clone()
    }
}

#[cfg(test)]
#[path = "floatfold_tests.rs"]
mod tests;
