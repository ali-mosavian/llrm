//! Port of `qbopt/optimize/loadjoins.py`: join memory values, completing
//! availability on non-speculative edges.

#![allow(private_interfaces)] // `RegionLayout` is regions' crate-private type.

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use crate::analysis::loops::{self, Loop};
use crate::analysis::regions::RegionLayout;
use crate::analysis::{memoryssa, ssa};
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Cell, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value};
use crate::objectfile::module::Space;
use crate::optimize::edges;

fn _on_edge(r#ref: &MemRef, phis: &[Phi], predecessor: i64) -> Option<MemRef> {
    if !r#ref.pointer {
        return Some(r#ref.clone());
    }
    for phi in phis {
        if Some(phi.result) == r#ref.base {
            let value = phi.incoming.get(&predecessor);
            return value.map(|value| MemRef { base: Some(*value), ..r#ref.clone() });
        }
    }
    Some(r#ref.clone())
}

fn _value(op: &Op) -> Option<(MemRef, Held)> {
    if op.barrier() || op.floating.is_some() || op.stack.is_some() || !op.merges.is_empty() {
        return None;
    }
    let (r#ref, value) = match (op.kind, &op.args[..], &op.results[..]) {
        (Kind::Load, [Arg::Cell(Cell { r#ref })], [Arg::Held(value)])
            if op.stores.is_empty() && op.loads == [r#ref.clone()] =>
        {
            if op.defines != [value.value] {
                return None;
            }
            (r#ref, value)
        }
        (Kind::Store, [Arg::Held(value)], [Arg::Cell(Cell { r#ref })])
            if op.loads.is_empty() && op.stores == [r#ref.clone()] =>
        {
            if !op.defines.is_empty() {
                return None;
            }
            (r#ref, value)
        }
        _ => return None,
    };
    if value.width != r#ref.width
        || (r#ref.addr.is_none() && !r#ref.pointer)
        || r#ref.addr.is_some_and(|addr| addr.space == Space::Stack)
    {
        return None;
    }
    Some((r#ref.clone(), *value))
}

#[allow(clippy::too_many_arguments)]
fn _insertion(
    parent: &MirBlock,
    join: &MirBlock,
    index: usize,
    translated: &MemRef,
    op: &Op,
    definitions: &IndexMap<Value, (i64, i64)>,
    dominators: &BTreeMap<i64, BTreeSet<i64>>,
    natural_loops: &[Loop],
) -> Option<(usize, Vec<Value>)> {
    let critical = edges::explicit(parent, join.at);
    if (parent.succ != [join.at] && !critical)
        || dominators[&parent.at].contains(&join.at)
        || natural_loops.iter().any(|one| one.body.contains(&parent.at) != one.body.contains(&join.at))
    {
        return None;
    }
    for prior in &join.ops[..index] {
        if !matches!(prior.kind, Kind::Nothing | Kind::Copy)
            || prior.barrier()
            || prior.floating.is_some()
            || !prior.loads.is_empty()
            || !prior.stores.is_empty()
            || prior.stack.is_some()
            || prior.args.iter().any(|arg| matches!(arg, Arg::Cell(_)))
        {
            return None;
        }
    }
    let incoming: IndexMap<Value, Option<Value>> =
        join.phis.iter().map(|phi| (phi.result, phi.incoming.get(&parent.at).copied())).collect();
    let uses: Vec<Option<Value>> =
        op.uses.iter().map(|value| incoming.get(value).copied().unwrap_or(Some(*value))).collect();
    let address_uses: Vec<Option<Value>> = [translated.base, translated.segment].into_iter().flatten().map(Some).collect();
    let mut deduplicated: Vec<Option<Value>> = Vec::new();
    for value in uses.into_iter().chain(address_uses) {
        if !deduplicated.contains(&value) {
            deduplicated.push(value);
        }
    }
    let uses = deduplicated;
    let mut cut = parent.ops.len();
    if cut > 0 && matches!(parent.ops[cut - 1].kind, Kind::Jump | Kind::Branch) {
        cut -= 1;
    }
    if parent.ops[..cut].iter().any(|prior| matches!(prior.kind, Kind::Branch | Kind::Return | Kind::Escape)) {
        return None;
    }
    if parent.ops.last().is_some_and(|last| matches!(last.kind, Kind::Return | Kind::Escape)) {
        return None;
    }
    let mut values = Vec::new();
    for value in uses {
        let Some(value) = value else {
            return None;
        };
        if value.flags || !definitions.contains_key(&value) {
            return None;
        }
        let (block, position) = definitions[&value];
        if !dominators[&parent.at].contains(&block) || block == parent.at && position >= cut as i64 {
            return None;
        }
        values.push(value);
    }
    Some((cut, values))
}

pub fn reused(body: &MirBody, dgroup: Option<&RegionLayout>, insert: bool) -> Result<MirBody, String> {
    let predecessors = loops::predecessors(&body.blocks);
    if !predecessors.values().any(|parents| parents.len() > 1) {
        return Ok(body.clone());
    }
    let graph = memoryssa::built(body);
    let providers: Vec<(memoryssa::Site, (MemRef, Held))> =
        graph.operations.iter().filter_map(|(site, op)| _value(op).map(|value| (*site, value))).collect();
    let dominators = loops::dominators(&body.blocks, body.entry);
    let natural_loops = loops::loops(&body.blocks, Some(body.entry));
    let mut fresh = ssa::values(body).map(|value| value.id).max().unwrap_or(0) + 1;
    let by_at: IndexMap<i64, &MirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut definitions: IndexMap<Value, (i64, i64)> = body
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .ops
                .iter()
                .enumerate()
                .flat_map(move |(index, op)| op.defines.iter().map(move |value| (*value, (block.at, index as i64))))
        })
        .collect();
    definitions.extend(
        body.blocks.iter().flat_map(|block| block.phis.iter().map(move |phi| (phi.result, (block.at, -1)))),
    );
    let mut insertions: IndexMap<i64, Vec<(usize, Op)>> = IndexMap::new();
    let mut bridges: IndexMap<(i64, i64), (i64, Vec<Op>)> = IndexMap::new();
    let mut label = edges::fresh(body);
    let mut blocks: Vec<MirBlock> = Vec::new();
    for block in &body.blocks {
        let parents = &predecessors[&block.at];
        if block.at == body.entry || parents.len() < 2 {
            blocks.push(block.clone());
            continue;
        }
        let (mut phis, mut ops) = (block.phis.clone(), Vec::new());
        for (index, op) in block.ops.iter().enumerate() {
            let loaded = if op.kind == Kind::Load { _value(op) } else { None };
            let mut incoming: IndexMap<i64, Value> = IndexMap::new();
            let mut missing: IndexMap<i64, (usize, Vec<Value>, MemRef)> = IndexMap::new();
            let Some((r#ref, result)) = loaded else {
                ops.push(op.clone());
                continue;
            };
            let site = memoryssa::Site { block: block.at, index };
            for parent in parents {
                let parent = *parent;
                let Some(translated) = _on_edge(&r#ref, &block.phis, parent) else {
                    break;
                };
                let candidates: Vec<(memoryssa::Site, Held)> = providers
                    .iter()
                    .filter(|(source, (cell, value))| {
                        source.block != block.at
                            && dominators[&parent].contains(&source.block)
                            && !dominators[&source.block].contains(&block.at)
                            && value.width == result.width
                            && graph.pointers.same_bytes(cell, &translated)
                            && natural_loops
                                .iter()
                                .all(|one| !one.body.contains(&source.block) || one.body.contains(&block.at))
                            && graph.available_on_edge(*source, site, parent, &r#ref, dgroup, Some(&translated))
                    })
                    .map(|(source, (_, value))| (*source, *value))
                    .collect();
                if candidates.is_empty() {
                    let placement = if insert {
                        _insertion(
                            by_at[&parent],
                            block,
                            index,
                            &translated,
                            op,
                            &definitions,
                            &dominators,
                            &natural_loops,
                        )
                    } else {
                        None
                    };
                    let Some((cut, uses)) = placement else {
                        break;
                    };
                    missing.insert(parent, (cut, uses, translated));
                    continue;
                }
                // Python's `max` keeps the first of equal keys.
                let key = |item: &(memoryssa::Site, Held)| (dominators[&item.0.block].len(), item.0.index);
                let mut best = &candidates[0];
                for candidate in &candidates[1..] {
                    if key(candidate) > key(best) {
                        best = candidate;
                    }
                }
                incoming.insert(parent, best.1.value);
            }
            if incoming.is_empty() || incoming.len() + missing.len() != parents.len() {
                ops.push(op.clone());
                continue;
            }
            for (parent, (cut, uses, translated)) in missing {
                let predecessor = by_at[&parent];
                let mut at = if predecessor.ops.is_empty() {
                    parent
                } else {
                    predecessor.ops[cut.min(predecessor.ops.len() - 1)].at
                };
                let critical = edges::explicit(predecessor, block.at);
                let edge = (parent, block.at);
                if critical {
                    if !bridges.contains_key(&edge) {
                        bridges.insert(edge, (label, Vec::new()));
                        label += 1;
                    }
                    at = bridges[&edge].0;
                }
                let value = Value::new(fresh, at);
                fresh += 1;
                let mut load = Op::new(at, OpCode::Operation(Operation::Move), "", vec![value], uses);
                load.kind = Kind::Load;
                load.args = vec![Arg::Cell(Cell { r#ref: translated.clone() })];
                load.results = vec![Arg::Held(Held { value, width: result.width })];
                load.loads = vec![translated];
                load.symbol = Some(false);
                if critical {
                    bridges.get_mut(&edge).expect("inserted above").1.push(load);
                } else {
                    insertions.entry(parent).or_default().push((cut, load));
                }
                incoming.insert(parent, value);
            }
            let value = Value::new(fresh, block.at);
            fresh += 1;
            phis.push(Phi { result: value, incoming: incoming.into_iter().collect::<OrderedMap<_, _>>() });
            let mut copy = op.clone();
            copy.kind = Kind::Copy;
            copy.args = vec![Arg::Held(Held { value, width: result.width })];
            copy.uses = vec![value];
            copy.loads = Vec::new();
            copy.source_backed = false;
            copy.raised = None;
            copy.symbol = Some(false);
            ops.push(copy);
        }
        blocks.push(MirBlock { phis, ops, ..block.clone() });
    }
    for block in &mut blocks {
        let mut ordered = insertions.get(&block.at).cloned().unwrap_or_default();
        ordered.sort_by(|one, other| other.0.cmp(&one.0));
        for (cut, load) in ordered {
            block.ops.insert(cut, load);
        }
    }
    let mut result = MirBody { blocks, ..body.clone() };
    for ((parent, target), (label, loads)) in bridges {
        result = edges::split(&result, parent, target, label, loads)?;
    }
    Ok(result)
}

#[cfg(test)]
#[path = "loadjoins_tests.rs"]
mod tests;
