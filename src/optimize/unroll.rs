//! Bounded full unrolling of small exact-trip loops.
//!
//! Port of `qbopt/optimize/unroll.py`.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::analysis::loops::{self, Loop};
use crate::analysis::peelsize::{self, Signature};
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
        expanded(&body, &self.r#where, &self.r#where.named(), &BTreeSet::new(), None)
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
    tried: Option<&BTreeSet<Signature>>,
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
        if tried.is_some_and(|tried| tried.contains(&peelsize::signature(body, &loop_, &count, &facts))) {
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

fn _size(body: &MirBody) -> i64 {
    body.blocks
        .iter()
        .map(|block| {
            (block.phis.len() + block.ops.iter().filter(|op| op.kind != Kind::Nothing).count()) as i64
        })
        .sum()
}

/// Conservative semantic operations attributable to one expanded sequence.
fn _expanded_operations(before: &MirBody, after: &MirBody, latch: i64, count: i64) -> i64 {
    let found = loops::loops(&before.blocks, Some(before.entry))
        .into_iter()
        .filter(|one| one.latches.contains(&latch))
        .collect::<Vec<_>>();
    if found.len() != 1 {
        return _size(after);
    }
    let inside = &found[0].body;
    let loop_size = before
        .blocks
        .iter()
        .filter(|block| inside.contains(&block.at))
        .map(|block| {
            (block.phis.len() + block.ops.iter().filter(|op| op.kind != Kind::Nothing).count()) as i64
        })
        .sum::<i64>();
    let outside = 0.max(_size(before) - loop_size);
    let settled = 0.max(_size(after) - outside);
    settled.max(loop_size * count)
}

/// Whether exact dynamic savings pay for the optimized straight-line body.
fn _profitable(before: &MirBody, after: &MirBody, latch: i64, count: i64, r#where: &Where) -> bool {
    _rejection(before, after, latch, count, r#where).is_none()
}

/// Why a structural candidate loses, or `None` when it wins.
pub(crate) fn _rejection(
    before: &MirBody,
    after: &MirBody,
    latch: i64,
    count: i64,
    r#where: &Where,
) -> Option<&'static str> {
    if loops::loops(&after.blocks, Some(after.entry)).len() >= loops::loops(&before.blocks, Some(before.entry)).len() {
        return Some("residual-loops");
    }
    if !r#where.options.grows && _size(after) > _size(before) {
        return Some("size-growth");
    }
    if r#where.options.max_unroll_iterations != 0
        && count > r#where.options.max_unroll_iterations
        && _size(after) > _size(before)
    {
        // A large exact loop may still be an excellent constant-folding
        // vehicle: allow it when scalar optimization erases all expansion
        // growth. Otherwise obey the target's complete-peel budget before an
        // expensive branch makes arbitrary duplication look free.
        return Some("iteration-growth");
    }
    let trips = IndexMap::from_iter([(latch, count)]);
    let dynamic_before = profit::weighted(before, &r#where.costs, Some(&trips));
    let dynamic_after = profit::weighted(after, &r#where.costs, None);
    let (Some(dynamic_before), Some(dynamic_after)) = (dynamic_before, dynamic_after) else {
        return Some("unpriced");
    };
    if dynamic_after >= dynamic_before {
        return Some("no-saving");
    }
    let pressure_before = profit::spill_risk(before, &r#where.costs, r#where.registers, Some(&trips));
    let pressure_after = profit::spill_risk(after, &r#where.costs, r#where.registers, None);
    let (Some(pressure_before), Some(pressure_after)) = (pressure_before, pressure_after) else {
        return Some("unpriced");
    };
    let total_before = dynamic_before + pressure_before;
    let total_after = dynamic_after + pressure_after;
    let sequence = _expanded_operations(before, after, latch, count);
    if pressure_after > 0
        && r#where.options.max_unrolled_operations != 0
        && sequence > r#where.options.max_unrolled_operations
        && (pressure_after >= pressure_before || total_before - total_after <= sequence * r#where.costs.r#move)
    {
        // GCC's target-independent `max-completely-peeled-insns` is 200.
        // An oversized spill-prone candidate must both lower pressure and
        // save enough dynamic work to pay for its whole expanded sequence.
        return Some("operation-growth");
    }
    if total_after >= total_before {
        return Some("pressure");
    }
    // MIR cannot know final encoding bytes. Charge one register move per added
    // semantic operation, amortized over the exact executions only when the
    // candidate is not already predicted to spill.
    let mut growth = 0.max(_size(after) - _size(before)) * r#where.costs.r#move;
    if pressure_after == 0 {
        growth = (growth + count - 1).div_euclid(count);
    }
    if total_before - total_after <= growth {
        Some("growth")
    } else {
        None
    }
}

/// Repeatedly expand one profitable exact loop and re-run scalar MIR.
///
/// `tried` outlives this call: the fixed point asks every round, and a loop
/// it already rejected, unchanged, is not asked about again.
pub fn optimized(
    body: &Rc<MirBody>,
    r#where: &Where,
    optimize: &mut dyn FnMut(Rc<MirBody>) -> Result<Rc<MirBody>, String>,
    tried: &std::cell::RefCell<BTreeSet<Signature>>,
    mut watch: Option<&mut dyn FnMut(&str, &MirBody)>,
) -> Result<Rc<MirBody>, String> {
    if !priced(body, r#where) {
        return Ok(body.clone());
    }
    let mut body = body.clone();
    let mut rejected = BTreeSet::<i64>::new();
    loop {
        let candidate = expanded(&body, r#where, &r#where.named(), &rejected, Some(&tried.borrow()))?;
        if Rc::ptr_eq(&candidate, &body) {
            return Ok(body);
        }
        let additions = &candidate.repetitions[body.repetitions.len()..];
        if additions.len() != 1 {
            return Ok(body);
        }
        let (latch, count) = additions[0];
        if rejected.contains(&latch) {
            return Ok(body);
        }
        if let Some(watch) = watch.as_deref_mut() {
            watch("unroll-candidate", &candidate);
        }
        let result = optimize(candidate)?;
        if let Some(rejection) = _rejection(&body, &result, latch, count, r#where) {
            if let Some(watch) = watch.as_deref_mut() {
                watch(&format!("unroll-rejected-{rejection}"), &result);
            }
            rejected.insert(latch);
            tried.borrow_mut().insert(_signature(&body, latch, count, r#where));
            continue;
        }
        body = result;
        if let Some(watch) = watch.as_deref_mut() {
            watch("unroll-accepted", &body);
        }
        rejected.clear();
    }
}

/// Whether a candidate here could be accepted at all: `_rejection` prices both sides.
pub(crate) fn priced(body: &MirBody, r#where: &Where) -> bool {
    profit::r#static(body, &r#where.costs).is_some()
}

fn _signature(body: &Rc<MirBody>, latch: i64, count: i64, r#where: &Where) -> Signature {
    let found = loops::loops(&body.blocks, Some(body.entry))
        .into_iter()
        .filter(|one| one.latches.contains(&latch))
        .collect::<Vec<_>>();
    let [loop_] = found.as_slice() else {
        panic!("ValueError: expected one loop with latch {latch}, found {}", found.len());
    };
    peelsize::signature(
        body,
        loop_,
        &BigInt::from(count),
        &consts::known(body, Some(&r#where.dgroup), Some(&r#where.named()), None, None),
    )
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
