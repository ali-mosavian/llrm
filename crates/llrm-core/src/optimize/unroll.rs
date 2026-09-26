//! Bounded full unrolling of small exact-trip loops.
//!
//! Port of `qbopt/optimize/unroll.py`.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::analysis::loops::{self, Loop};
use crate::analysis::peelsize;
use crate::analysis::{consts, floatfacts, induction, ssa};
use crate::model::mir::{Arg, Kind, MirBlock, MirBody, Op, OrderedMap, Value};
use crate::model::passes::{MIRTransform, Where};
use crate::optimize::profit;

pub struct Unroll {
    pub r#where: Where,
}

impl Unroll {
    pub fn new(r#where: Where) -> Self {
        Self { r#where }
    }
}

impl MIRTransform for Unroll {
    fn class_name(&self) -> &'static str {
        "Unroll"
    }

    fn name(&self) -> &str {
        "unroll"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        expanded(&body, &self.r#where, &self.r#where.named(), &BTreeSet::new())
    }
}

fn substitution(error: ssa::SubstitutionError) -> String {
    error.to_string()
}

pub fn expanded(
    body: &Rc<MirBody>,
    r#where: &Where,
    calls: &IndexMap<i64, String>,
    skip: &BTreeSet<i64>,
) -> Result<Rc<MirBody>, String> {
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<i64, &MirBlock>>();
    let predecessors = loops::predecessors(&body.blocks);
    let dgroup = &r#where.dgroup;
    let facts = consts::known(body, Some(dgroup), Some(calls), None, None);
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        if loop_.latches.len() != 1 {
            continue;
        }
        let header = blocks[&loop_.header];
        let latch = blocks[loop_.latches.iter().next().expect("one latch")];
        if skip.contains(&latch.at) {
            continue;
        }
        let outside = predecessors[&header.at]
            .difference(&loop_.body)
            .copied()
            .collect::<Vec<_>>();
        let exits = header
            .succ
            .iter()
            .copied()
            .filter(|at| !loop_.body.contains(at))
            .collect::<BTreeSet<_>>();
        if outside.len() != 1 || exits.len() != 1 || !latch.phis.is_empty() {
            continue;
        }
        let entry = outside[0];
        let exit_at = *exits.first().expect("one exit");
        if predecessors[&exit_at] != BTreeSet::from([header.at])
            || blocks[&exit_at]
                .phis
                .iter()
                .any(|phi| phi.incoming.keys().copied().collect::<BTreeSet<_>>() != BTreeSet::from([header.at]))
        {
            continue;
        }
        let bridges = loop_
            .body
            .iter()
            .filter(|at| **at != header.at && **at != latch.at)
            .map(|at| blocks[at])
            .collect::<Vec<_>>();
        if bridges
            .iter()
            .any(|block| !block.phis.is_empty() || block.succ.len() != 1)
        {
            continue;
        }
        let mut path = Vec::new();
        let inside = header
            .succ
            .iter()
            .copied()
            .filter(|at| loop_.body.contains(at))
            .collect::<BTreeSet<_>>();
        let [mut at] = inside.into_iter().collect::<Vec<_>>()[..] else {
            panic!("not exactly one value to unpack");
        };
        while at != latch.at && !path.contains(&at) {
            path.push(at);
            at = blocks[&at].succ[0];
        }
        if at != latch.at
            || path.iter().copied().collect::<BTreeSet<_>>()
                != bridges.iter().map(|block| block.at).collect::<BTreeSet<_>>()
        {
            continue;
        }
        let ordered_bridges = path.iter().map(|at| blocks[at]).collect::<Vec<_>>();
        let bridge_ops = ordered_bridges
            .iter()
            .flat_map(|block| {
                let ops = &block.ops[..];
                if ops
                    .last()
                    .is_some_and(|last| last.kind == Kind::Jump && last.target == Some(block.succ[0]))
                {
                    &ops[..ops.len() - 1]
                } else {
                    ops
                }
            })
            .cloned()
            .collect::<Vec<Op>>();
        // The object frontend may express the backedge only in CFG while the
        // C frontend carries an explicit terminal JUMP.  They are the same
        // loop.  The jump is control, not one iteration's work, and the
        // expansion replaces it with one jump to the exit.
        let mut latch_ops = &latch.ops[..];
        if latch_ops
            .last()
            .is_some_and(|last| last.kind == Kind::Jump && last.target == Some(header.at))
        {
            latch_ops = &latch_ops[..latch_ops.len() - 1];
        }
        let latch_ops = latch_ops.to_vec();
        let repeated_ops = bridge_ops.iter().chain(latch_ops.iter()).collect::<Vec<_>>();
        let floating_loop = repeated_ops.iter().any(|op| op.floating.is_some());
        let invalid_latch = if floating_loop {
            repeated_ops.iter().any(|op| {
                op.barrier() || matches!(op.kind, Kind::Opaque | Kind::Branch | Kind::Jump)
            })
        } else {
            repeated_ops.iter().any(|op| {
                op.barrier()
                    || matches!(
                        op.kind,
                        Kind::Opaque | Kind::Call | Kind::Return | Kind::Branch | Kind::Jump | Kind::Switch
                    )
            })
        };
        if invalid_latch {
            continue;
        }
        if header.ops.last().is_none_or(|last| last.kind != Kind::Branch) {
            continue;
        }
        // Keep the established floating-loop contract: compiler bookkeeping
        // may store its counter in the test block, but no floating operation
        // may be repeated there.  A new integer expansion accepts any pure
        // value computation and refuses observable memory/control effects.
        let invalid_header = if floating_loop {
            header.ops.iter().any(|op| {
                !matches!(op.kind, Kind::Nothing | Kind::Store | Kind::Sub | Kind::Branch)
                    || op.barrier()
                    || op.floating.is_some()
            })
        } else {
            header.ops[..header.ops.len() - 1].iter().any(|op| {
                op.barrier()
                    || !op.stores.is_empty()
                    || matches!(
                        op.kind,
                        Kind::Opaque
                            | Kind::Call
                            | Kind::Return
                            | Kind::Branch
                            | Kind::Jump
                            | Kind::Switch
                            | Kind::Arg
                            | Kind::Result
                            | Kind::Escape
                    )
            })
        };
        if invalid_header {
            continue;
        }
        let Some(count) = induction::trip_count(body, &loop_, &facts) else {
            continue;
        };
        if count < BigInt::from(2) || !peelsize::admitted(body, &loop_, &count, &facts, r#where) {
            continue;
        }
        let count = count.to_i64().expect("count fits");
        if header.phis.iter().any(|phi| {
            phi.incoming.keys().copied().collect::<BTreeSet<_>>() != BTreeSet::from([entry, latch.at])
        }) {
            continue;
        }
        let candidate = Rc::new(_expanded(body, &loop_, header, latch, &bridge_ops, &latch_ops, exit_at, entry, count)?);
        if count > 4 && floating_loop {
            let exact = floatfacts::known(&candidate, dgroup, calls, None);
            let results = candidate
                .block(latch.at)
                .expect("the latch survives")
                .ops
                .iter()
                .filter(|op| op.floating.is_some())
                .flat_map(|op| op.results.iter())
                .filter_map(|arg| match arg {
                    Arg::Held(held) if held.width == 10 => Some(held.value),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if results.is_empty() || results.iter().any(|value| !exact.contains_key(value)) {
                continue;
            }
        }
        return Ok(candidate);
    }
    Ok(body.clone())
}

/// Expand every exact loop `peelsize::admitted` prices as worth it, once each; the
/// caller's fixed point settles the copies.
pub fn optimized(
    body: &Rc<MirBody>,
    r#where: &Where,
    mut watch: Option<&mut dyn FnMut(&str, &MirBody)>,
) -> Result<crate::model::mir::Transformed, String> {
    let mut body = body.clone();
    let mut stages = Vec::new();
    if !priced(&body, r#where) {
        return Ok(crate::model::mir::Transformed { body, stages });
    }
    loop {
        let candidate = expanded(&body, r#where, &r#where.named(), &BTreeSet::new())?;
        if Rc::ptr_eq(&candidate, &body) {
            return Ok(crate::model::mir::Transformed { body, stages });
        }
        let (candidate, stage) = crate::model::mir::transformed(&body, candidate);
        stages.push(crate::model::mir::Stage { name: "unroll-accepted".to_owned(), ..stage });
        if let Some(watch) = watch.as_deref_mut() {
            watch("unroll-accepted", &candidate);
        }
        llrm_support::debug!("unroll", "expanded; {} copies so far", candidate.repetitions.len());
        body = candidate;
    }
}

/// Whether the target prices every operation here, which a copy's cost needs.
pub fn priced(body: &MirBody, r#where: &Where) -> bool {
    profit::r#static(body, &r#where.costs).is_some()
}

#[allow(clippy::too_many_arguments)]
fn _expanded(
    body: &MirBody,
    loop_: &Loop,
    header: &MirBlock,
    latch: &MirBlock,
    bridge_ops: &[Op],
    latch_ops: &[Op],
    exit_at: i64,
    entry: i64,
    count: i64,
) -> Result<MirBody, String> {
    let values = ssa::values(body).collect::<Vec<_>>();
    let mut next_id = values.iter().map(|value| value.id).max().expect("max() arg is an empty sequence") + 1;
    let mut next_variable = values
        .iter()
        .map(|value| value.variable)
        .max()
        .expect("max() arg is an empty sequence")
        + 1;
    let mut swap = header
        .phis
        .iter()
        .map(|phi| (phi.result.id, *phi.incoming.get(&entry).expect("KeyError")))
        .collect::<BTreeMap<u32, Value>>();
    let initial = swap.clone();
    let mut copies = Vec::<IndexMap<u32, Value>>::new();

    let mut clone = |op: &Op, owns: bool, swap: &mut BTreeMap<u32, Value>| -> Result<Op, String> {
        let read = ssa::substituted(op, swap).map_err(substitution)?;
        let mut defined = IndexMap::<u32, Value>::default();
        for value in &op.defines {
            let fresh = Value {
                id: next_id,
                variable: next_variable,
                version: 1,
                ..*value
            };
            next_id += 1;
            next_variable += 1;
            defined.insert(value.id, fresh);
        }
        copies.push(defined.clone());
        swap.extend(defined.iter().map(|(id, value)| (*id, *value)));
        let results = read
            .results
            .iter()
            .map(|arg| match arg {
                Arg::Held(held) => Arg::Held(crate::model::mir::Held {
                    value: defined.get(&held.value.id).copied().unwrap_or(held.value),
                    width: held.width,
                }),
                _ => arg.clone(),
            })
            .collect();
        let mut cloned = read;
        cloned.defines = op.defines.iter().map(|value| defined[&value.id]).collect();
        cloned.results = results;
        cloned.absorbed = if owns { op.absorbed.clone() } else { Vec::new() };
        cloned.raised = None;
        cloned.symbol = if owns { op.symbol } else { Some(op.symbol != Some(false)) };
        Ok(cloned)
    };

    let mut expanded = Vec::new();
    for iteration in 0..count {
        if iteration != 0 {
            for op in &header.ops[..header.ops.len() - 1] {
                expanded.push(clone(op, false, &mut swap)?);
            }
            for op in bridge_ops {
                expanded.push(clone(op, false, &mut swap)?);
            }
        }
        for op in latch_ops {
            expanded.push(clone(op, iteration == 0, &mut swap)?);
        }
        let mut carried = BTreeMap::new();
        for phi in &header.phis {
            let incoming = *phi.incoming.get(&latch.at).expect("KeyError");
            carried.insert(phi.result.id, ssa::provider(incoming, &swap).map_err(substitution)?);
        }
        swap.extend(carried);
    }
    for op in &header.ops[..header.ops.len() - 1] {
        expanded.push(clone(op, false, &mut swap)?);
    }
    let anchor = latch.ops.last().expect("IndexError").at;
    let mut leave = header.ops.last().expect("a branch ends the header").clone();
    leave.at = anchor;
    leave.kind = Kind::Jump;
    leave.name = String::new();
    leave.args = Vec::new();
    leave.results = Vec::new();
    leave.uses = Vec::new();
    leave.defines = Vec::new();
    leave.loads = Vec::new();
    leave.stores = Vec::new();
    leave.merges = OrderedMap::new();
    leave.source_backed = false;
    leave.raised = Some((Vec::new(), Vec::new()));
    leave.absorbed = Vec::new();
    leave.target = Some(exit_at);
    leave.test = None;
    leave.symbol = Some(false);
    expanded.push(leave);
    let mut changed = Vec::new();
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let inside = header
        .succ
        .iter()
        .copied()
        .filter(|at| loop_.body.contains(at))
        .collect::<BTreeSet<_>>();
    let [first_iteration] = inside.into_iter().collect::<Vec<_>>()[..] else {
        panic!("not exactly one value to unpack");
    };
    let empty = BTreeSet::new();
    for block in &body.blocks {
        let mut block = block.clone();
        if block.at == header.at {
            let ops = block
                .ops
                .iter()
                .map(|op| ssa::substituted(op, &initial).map_err(substitution))
                .collect::<Result<Vec<_>, _>>()?;
            // `trip_count` established a positive exact count before this
            // expansion was built.  The original zero-trip edge is therefore
            // impossible: keep the source occurrence as an unconditional
            // control anchor; ordinary CFG cleanup removes it.
            let guard = ops.last().expect("IndexError");
            let mut enter = guard.clone();
            enter.kind = Kind::Jump;
            enter.name = String::new();
            enter.args = Vec::new();
            enter.results = Vec::new();
            enter.uses = Vec::new();
            enter.defines = Vec::new();
            enter.loads = Vec::new();
            enter.stores = Vec::new();
            enter.merges = OrderedMap::new();
            enter.source_backed = false;
            enter.raised = Some((Vec::new(), Vec::new()));
            enter.absorbed = guard.absorbed.clone();
            enter.target = Some(first_iteration);
            enter.test = None;
            enter.symbol = Some(false);
            let mut ops = ops[..ops.len() - 1].to_vec();
            ops.push(enter);
            block.phis = Vec::new();
            block.ops = ops;
            block.succ = vec![first_iteration];
        } else if block.at == latch.at {
            block.ops = expanded.clone();
            block.succ = vec![exit_at];
        } else if loop_.body.contains(&block.at) {
            // The first iteration still reaches the original straight-line
            // bridge blocks, which must read the entry values once the header
            // phis are removed.
            block.ops = block
                .ops
                .iter()
                .map(|op| ssa::substituted(op, &initial).map_err(substitution))
                .collect::<Result<Vec<_>, _>>()?;
        } else {
            // A phi reads on its incoming edge, not in the block containing
            // it; substitute precisely the edge uses the exit dominates.
            let mut phis = Vec::new();
            for phi in &block.phis {
                let mut phi = phi.clone();
                let mut incoming = OrderedMap::new();
                for (at, value) in phi.incoming.iter() {
                    let value = if dominators.get(at).unwrap_or(&empty).contains(&exit_at) {
                        ssa::provider(*value, &swap).map_err(substitution)?
                    } else {
                        *value
                    };
                    incoming.insert(*at, value);
                }
                phi.incoming = incoming;
                phis.push(phi);
            }
            if block.at == exit_at {
                // A positive exact trip count removed the zero-trip edge.  The
                // expanded latch is the exit's sole remaining predecessor.
                phis = Vec::new();
                for phi in &block.phis {
                    let mut phi = phi.clone();
                    let from = *phi.incoming.get(&header.at).expect("KeyError");
                    phi.incoming =
                        OrderedMap::from_iter([(latch.at, ssa::provider(from, &swap).map_err(substitution)?)]);
                    phis.push(phi);
                }
            }
            if dominators.get(&block.at).unwrap_or(&empty).contains(&exit_at) {
                block.ops = block
                    .ops
                    .iter()
                    .map(|op| ssa::substituted(op, &swap).map_err(substitution))
                    .collect::<Result<Vec<_>, _>>()?;
            }
            block.phis = phis;
        }
        changed.push(block);
    }
    let (pointer_values, pointer_seeds) = ssa::cloned_pointer_metadata(body, copies.iter());
    let integer_ranges = ssa::cloned_integer_ranges(body, copies.iter());
    let mut repetitions = body.repetitions.clone();
    repetitions.push((latch.at, count));
    Ok(MirBody { repetitions, pointer_values, pointer_seeds, integer_ranges, ..body.with_blocks(changed) })
}

#[cfg(test)]
#[path = "unroll_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "unroll_budget_tests.rs"]
mod budget_tests;
