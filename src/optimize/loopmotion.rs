//! Sink loop stores nothing inside the loop observes to its single exit.
//!
//! Port of `qbopt/optimize/loopmotion.py`.

// Its callers live in transform.py, not yet ported.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use indexmap::IndexMap;

use crate::analysis::consts::{self, Cells, Known};
use crate::analysis::loops::{self, Loop};
use crate::analysis::ranges::{self, Interval};
use crate::analysis::regions::{self, RegionError, RegionLayout};
use crate::analysis::{effects, induction};
use crate::model::mir::{Arg, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, Value};
use crate::objectfile::module::Space;
use crate::support::pyset::PySet;

/// Python's `bounds` dict, as the region layout that carries it.
type Bounds = IndexMap<(Space, i64), Vec<i64>>;

/// `id(op)`.
fn id(op: &Op) -> usize {
    std::ptr::from_ref(op) as usize
}

fn layout(bounds: Option<&Bounds>) -> Option<RegionLayout> {
    bounds.map(|bounds| RegionLayout {
        shared_segments: None,
        landmarks: bounds.iter().map(|(key, marks)| (*key, marks.clone())).collect(),
    })
}

/// `mir.overlapping(one, other, dgroup, bounds, known, other_known)`.
fn overlapping(
    one: &MemRef,
    other: &MemRef,
    bounds: Option<&Bounds>,
    known: Option<&BTreeMap<Value, Interval>>,
    other_known: Option<&BTreeMap<Value, Interval>>,
) -> Result<bool, String> {
    regions::overlapping(one, other, known, other_known, layout(bounds).as_ref())
        .map_err(|error: RegionError| format!("{error:?}"))
}

pub fn sunk_stores(
    body: &Rc<MirBody>,
    dgroup: &BTreeSet<i64>,
    bounds: Option<&Bounds>,
    handles_errors: bool,
) -> Result<Rc<MirBody>, String> {
    let mut body = body.clone();
    let predecessors = loops::predecessors(&body.blocks);
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let scoped = ranges::bounded(&body)?;
        // With constants, as hoist asks: a store through the literal selector
        // 0A000h otherwise observes every frame and descriptor cell.
        let constant: BTreeMap<Value, Interval> = ranges::constants(&body, Some(dgroup), None).into_iter().collect();
        let mut intervals = HashMap::<usize, BTreeMap<Value, Interval>>::new();
        for block in &body.blocks {
            let mut here = constant.clone();
            if let Some(found) = scoped.get(&block.at) {
                here.extend(found.iter().map(|(value, interval)| (*value, interval.clone())));
            }
            for op in &block.ops {
                intervals.insert(id(op), here.clone());
            }
        }
        let blocks = body
            .blocks
            .iter()
            .map(|block| (block.at, block))
            .collect::<BTreeMap<i64, &MirBlock>>();
        // Python iterates `loop.body`, a frozenset, and the order reaches `moved`.
        let inside = loop_
            .body
            .iter()
            .copied()
            .collect::<PySet<i64>>()
            .iter()
            .map(|at| blocks[at])
            .collect::<Vec<_>>();
        let mut exits = Vec::new();
        for block in &inside {
            for to in &block.succ {
                if !loop_.body.contains(to) && !exits.contains(&(block.at, *to)) {
                    exits.push((block.at, *to));
                }
            }
        }
        if exits.len() != 1 {
            continue;
        }
        let (source, destination) = exits[0];
        if !blocks.contains_key(&destination) || predecessors[&destination] != BTreeSet::from([source]) {
            continue;
        }
        if inside.iter().any(|block| block.succ.is_empty()) {
            continue;
        }
        let operations = inside
            .iter()
            .flat_map(|block| block.ops.iter())
            .collect::<Vec<&Op>>();
        if operations.iter().any(|op| {
            effects::exposes_memory(op, handles_errors)
                || op.barrier()
                || matches!(
                    op.kind,
                    Kind::Call | Kind::Arg | Kind::Opaque | Kind::Escape | Kind::Return
                )
        }) {
            continue;
        }
        let exit_block = blocks[&destination];
        if exit_block.ops.is_empty() {
            continue;
        }
        let dominators = loops::dominators(&body.blocks, Some(body.entry));
        let empty = BTreeSet::new();
        let header_dominators = dominators.get(&loop_.header).unwrap_or(&empty);
        let address_values = body
            .blocks
            .iter()
            .filter(|block| !loop_.body.contains(&block.at) && header_dominators.contains(&block.at))
            .flat_map(|block| {
                block
                    .phis
                    .iter()
                    .map(|phi| phi.result)
                    .chain(block.ops.iter().flat_map(|op| op.defines.iter().copied()))
            })
            .collect::<BTreeSet<Value>>();
        let mut moved = Vec::<&Op>::new();
        for op in &blocks[&source].ops {
            if _unobserved(op, &operations, dgroup, bounds, Some(&intervals), &address_values)? {
                moved.push(op);
            }
        }
        let mut relocated = moved
            .iter()
            .map(|op| (id(op), (*op).clone()))
            .collect::<HashMap<usize, Op>>();
        if loop_.latches.len() == 1 && source == loop_.header {
            let latch = *loop_.latches.iter().next().expect("one latch");
            let outside = predecessors[&source]
                .difference(&loop_.body)
                .copied()
                .collect::<Vec<_>>();
            if outside.len() == 1 && latch != source && blocks[&latch].succ == [source] {
                let entry = outside[0];
                let nonempty = induction::nonempty(&body, &loop_);
                let invariant = if nonempty {
                    induction::invariant(&body, &loop_.body)
                } else {
                    BTreeSet::new()
                };
                for op in operations.iter().copied() {
                    if !relocated.contains_key(&id(op))
                        && _unobserved(op, &operations, dgroup, bounds, Some(&intervals), &address_values)?
                    {
                        let mut value = _exit_value(op, blocks[&source], blocks[&entry], latch, &body, dgroup, bounds, handles_errors)?
                            .map(Arg::Held);
                        if value.is_none() && blocks[&latch].ops.iter().any(|one| std::ptr::eq(op, one)) {
                            value = _invariant_value(op, &invariant, nonempty);
                            if value.is_none() {
                                value = _last_counter_value(op, &body, &loop_).map(Arg::Const);
                            }
                        }
                        if let Some(value) = value {
                            moved.push(op);
                            let mut changed = op.clone();
                            changed.args = op
                                .args
                                .iter()
                                .map(|arg| if matches!(arg, Arg::Held(_)) { value.clone() } else { arg.clone() })
                                .collect();
                            let mut uses = Vec::new();
                            if let Arg::Held(held) = &value {
                                uses.push(held.value);
                            }
                            for reference in &op.stores {
                                uses.extend([reference.base, reference.segment].into_iter().flatten());
                            }
                            let mut unique = Vec::new();
                            for one in uses {
                                if !unique.contains(&one) {
                                    unique.push(one);
                                }
                            }
                            changed.uses = unique;
                            relocated.insert(id(op), changed);
                        }
                    }
                }
            }
        }
        if moved.is_empty() {
            continue;
        }
        let identities = moved.iter().map(|op| id(op)).collect::<BTreeSet<usize>>();
        let mut updates = IndexMap::<i64, MirBlock>::new();
        for block in &inside {
            let mut changed = (*block).clone();
            changed.ops = block
                .ops
                .iter()
                .filter(|op| !identities.contains(&id(op)))
                .cloned()
                .collect();
            updates.insert(block.at, changed);
        }
        let mut changed = exit_block.clone();
        let anchor = exit_block.ops[0].at;
        changed.ops = moved
            .iter()
            .map(|op| Op {
                at: anchor,
                ..relocated[&id(op)].clone()
            })
            .chain(exit_block.ops.iter().cloned())
            .collect();
        updates.insert(destination, changed);
        let blocks = body
            .blocks
            .iter()
            .map(|block| updates.get(&block.at).cloned().unwrap_or_else(|| block.clone()))
            .collect();
        body = Rc::new(MirBody { blocks, ..MirBody::clone(&body) });
    }
    Ok(body)
}

fn _last_counter_value(op: &Op, body: &Rc<MirBody>, loop_: &Loop) -> Option<Const> {
    let [Arg::Held(stored)] = op.args.as_slice() else {
        return None;
    };
    let counter = induction::basics(body, loop_).get(&stored.value.id)?.clone();
    if counter.start.width() != stored.width || op.stores[0].width != stored.width {
        return None;
    }
    let last = induction::_last_counter(body, loop_, &counter, &consts::known(body, None, None, None, None), stored.width)?;
    Some(Const::new(consts::masked(&last, stored.width), stored.width))
}

fn _invariant_value(op: &Op, invariant: &BTreeSet<u32>, nonempty: bool) -> Option<Arg> {
    if !nonempty || op.args.len() != 1 {
        return None;
    }
    match &op.args[0] {
        Arg::Const(value) => Some(Arg::Const(value.clone())),
        Arg::Held(arg) if invariant.contains(&arg.value.id) => Some(Arg::Held(*arg)),
        _ => None,
    }
}

/// What `_exit_value` closes over.
struct _Exit<'a> {
    body: &'a MirBody,
    dgroup: &'a BTreeSet<i64>,
    bounds: Option<&'a Bounds>,
    reference: &'a MemRef,
    definitions: BTreeMap<Value, &'a Op>,
    blocks: BTreeMap<i64, &'a MirBlock>,
    predecessors: BTreeMap<i64, BTreeSet<i64>>,
    memory: Option<(IndexMap<(i64, usize), Cells>, IndexMap<i64, String>)>,
    handles_errors: bool,
}

impl _Exit<'_> {
    fn root(&self, mut arg: Arg) -> Arg {
        let mut seen = BTreeSet::new();
        while let Arg::Held(held) = &arg {
            if seen.contains(&held.value) {
                break;
            }
            seen.insert(held.value);
            let Some(defining) = self.definitions.get(&held.value) else {
                break;
            };
            if defining.kind != Kind::Copy || defining.args.len() != 1 {
                break;
            }
            if !defining
                .results
                .iter()
                .any(|result| matches!(result, Arg::Held(result) if result.value == held.value && result.width >= held.width))
            {
                break;
            }
            let width = held.width;
            arg = match &defining.args[0] {
                Arg::Held(source) if source.width >= width => Arg::Held(Held {
                    value: source.value,
                    width,
                }),
                Arg::Const(source) if source.width >= width => Arg::Const(Const::new(
                    &source.n & ((num_bigint::BigInt::from(1) << (width * 8)) - 1),
                    width,
                )),
                _ => break,
            };
        }
        arg
    }

    fn memory(&mut self) -> Result<&(IndexMap<(i64, usize), Cells>, IndexMap<i64, String>), String> {
        if self.memory.is_none() {
            let barriers = self
                .body
                .blocks
                .iter()
                .flat_map(|block| block.ops.iter())
                .filter(|one| one.barrier() || matches!(one.kind, Kind::Call | Kind::Escape | Kind::Opaque))
                .map(|one| (one.at, String::new()))
                .collect::<IndexMap<i64, String>>();
            let cells = consts::cells(self.body, self.dgroup, &barriers, None, None, None, None, None);
            self.memory = Some((cells, barriers));
        }
        Ok(self.memory.as_ref().expect("computed"))
    }

    fn stored_at(&mut self, at: i64, expected: Arg, active: &[(i64, Arg)]) -> Result<bool, String> {
        let expected = self.root(expected);
        let key = (at, expected.clone());
        if active.contains(&key) {
            return Ok(true); // inductive backedge; every entry path still needs a matching store
        }
        let block = self.blocks[&at];
        for index in (0..block.ops.len()).rev() {
            let previous = &block.ops[index];
            // A call is an operation like any other where its memory effects
            // are complete: its stores say what it can write.
            let opaque = previous.kind == Kind::Call && !previous.memory_complete;
            if previous.barrier() || opaque || matches!(previous.kind, Kind::Escape | Kind::Opaque) {
                return Ok(false);
            }
            if effects::exposes_memory(previous, self.handles_errors) {
                return Ok(false);
            }
            let mut overlaps = false;
            for written in &previous.stores {
                if overlapping(self.reference, written, self.bounds, None, None)? {
                    overlaps = true;
                    break;
                }
            }
            if overlaps {
                if let Arg::Const(expected) = &expected {
                    let mut fact = consts::initialized(previous, self.reference);
                    if fact.is_none() {
                        let (body, dgroup) = (self.body, self.dgroup);
                        let (facts, barriers) = self.memory()?;
                        let before = facts.get(&(at, index)).cloned().unwrap_or_default();
                        let nothing = IndexMap::new();
                        let mut asked = consts::memory_queries(body, &nothing, dgroup);
                        let after = consts::_kills(
                            &before,
                            previous,
                            &nothing,
                            dgroup,
                            barriers,
                            None,
                            None,
                            false,
                            Some(&mut asked),
                        );
                        fact = consts::_cell(&after, self.reference);
                    }
                    if fact == Some(Known::new(consts::masked(&expected.n, expected.width), expected.width)) {
                        return Ok(true);
                    }
                }
                let args = previous
                    .args
                    .iter()
                    .filter(|arg| matches!(arg, Arg::Const(_) | Arg::Held(_)))
                    .collect::<Vec<_>>();
                return Ok(previous.kind == Kind::Store
                    && previous.stores == [self.reference.clone()]
                    && args.len() == 1
                    && self.root(args[0].clone()) == expected);
            }
        }
        if at == self.body.entry || self.predecessors[&at].is_empty() {
            return Ok(false);
        }
        let phi = block
            .phis
            .iter()
            .find(|phi| matches!(&expected, Arg::Held(expected) if phi.result == expected.value));
        if let Some(phi) = phi {
            if phi.incoming.keys().copied().collect::<BTreeSet<_>>() != self.predecessors[&at] {
                return Ok(false);
            }
        }
        let mut active = active.to_vec();
        active.push(key);
        for parent in self.predecessors[&at].clone() {
            let next = match (phi, &expected) {
                (Some(phi), Arg::Held(expected)) => Arg::Held(Held {
                    value: *phi.incoming.get(&parent).expect("phi covers every predecessor"),
                    width: expected.width,
                }),
                _ => expected.clone(),
            };
            if !self.stored_at(parent, next, &active)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn _exit_value(
    op: &Op,
    header: &MirBlock,
    entry: &MirBlock,
    latch: i64,
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    bounds: Option<&Bounds>,
    handles_errors: bool,
) -> Result<Option<Held>, String> {
    let values = op
        .args
        .iter()
        .filter_map(|arg| match arg {
            Arg::Held(held) => Some(*held),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [stored] = values.as_slice() else {
        return Ok(None);
    };
    let definitions = body
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter())
        .flat_map(|operation| operation.defines.iter().map(move |value| (*value, operation)))
        .collect::<BTreeMap<Value, &Op>>();
    let mut exit = _Exit {
        body,
        dgroup,
        bounds,
        reference: &op.stores[0],
        definitions,
        blocks: body.blocks.iter().map(|block| (block.at, block)).collect(),
        predecessors: loops::predecessors(&body.blocks),
        memory: None,
        handles_errors,
    };

    for phi in &header.phis {
        if phi.incoming.keys().copied().collect::<BTreeSet<_>>() != BTreeSet::from([entry.at, latch]) {
            continue;
        }
        let seed = exit.root(Arg::Held(Held {
            value: phi.incoming.get(&entry.at).copied().expect("incoming from entry"),
            width: stored.width,
        }));
        if exit.stored_at(entry.at, seed, &[])?
            && exit.stored_at(
                latch,
                Arg::Held(Held {
                    value: phi.incoming.get(&latch).copied().expect("incoming from latch"),
                    width: stored.width,
                }),
                &[],
            )?
        {
            return Ok(Some(Held {
                value: phi.result,
                width: stored.width,
            }));
        }
    }
    Ok(None)
}

fn _unobserved(
    op: &Op,
    operations: &[&Op],
    dgroup: &BTreeSet<i64>,
    bounds: Option<&Bounds>,
    intervals: Option<&HashMap<usize, BTreeMap<Value, Interval>>>,
    address_values: &BTreeSet<Value>,
) -> Result<bool, String> {
    let _ = dgroup;
    if op.kind != Kind::Store || !op.loads.is_empty() || !op.defines.is_empty() || op.stores.len() != 1 {
        return Ok(false);
    }
    let reference = &op.stores[0];
    if reference
        .addr
        .is_none_or(|addr| !matches!(addr.space, Space::Segment | Space::Frame))
        || reference.segment.is_some()
    {
        return Ok(false);
    }
    if reference
        .base
        .is_some_and(|base| !address_values.contains(&base) || reference.excludes.is_empty())
    {
        return Ok(false);
    }
    let known = intervals.and_then(|intervals| intervals.get(&id(op)));
    for one in operations {
        if std::ptr::eq(*one, op) {
            continue;
        }
        let other_known = intervals.and_then(|intervals| intervals.get(&id(one)));
        for other in one.loads.iter().chain(one.stores.iter()) {
            if overlapping(reference, other, bounds, known, other_known)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "loopmotion_tests.rs"]
mod tests;
