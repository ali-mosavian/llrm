//! Enter a loop at its body after its entry has been proven separately.
//!
//! Direct port of `qbopt/optimize/rotate.py:at_body` and `_swapped`.
//! `analysis::induction` establishes whether a rotation is legal; this file
//! only reconstructs the proven MIR shape.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::induction::AffineOperand;
use crate::analysis::loops::{self, Loop};
use crate::analysis::ssa::SubstitutionError;
use crate::analysis::{consts, induction, occurrence, ssa};
use crate::model::mir::{
    self, Arg, Const, Held, Kind, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value,
};

use super::{cfg, transform};

/// Rotate a dead `0..bound-1` counter into a guarded countdown.
///
/// Direct port of `qbopt/optimize/rotate.py:_counted_down`.  Loop legality
/// and every source-counter observation are proved in `analysis::induction`;
/// this function consumes that one proof and reconstructs its replacement.
pub(crate) fn counted_down(body: &MirBody) -> Result<MirBody, SubstitutionError> {
    // Keep Python's one immutable fact calculation per recursive snapshot.
    let facts = consts::known(body, None, None, None, None);
    // Python evaluates this comprehension before selecting a loop.  It also
    // deliberately preserves duplicates for the max calculations below.
    let all_values = ssa::values(body).collect::<Vec<_>>();
    // `{block.at: block ...}` retains the last duplicate address.
    let blocks = body
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.at, index))
        .collect::<BTreeMap<_, _>>();
    // `{value.id: op ...}` has the same last-definition behavior.  An
    // occurrence is Python's operation object identity for this snapshot.
    let made = occurrence::operations(body)
        .flat_map(|(occurrence, _, operation)| {
            operation
                .defines
                .iter()
                .map(move |value| (value.id, occurrence))
        })
        .collect::<BTreeMap<_, _>>();

    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let proofs = induction::counted(body, &loop_, Some(&facts));
        if proofs.len() != 1 {
            continue;
        }
        let proof = &proofs[0];
        let Some(replacement) =
            induction::control_replacement(body, &loop_, proof, &BTreeSet::new())
        else {
            continue;
        };
        let AffineOperand::Held(bound) = &proof.bound else {
            // A constant count belongs to the ordinary finite-domain work.
            continue;
        };

        let preheader = proof.preheader;
        let latch_at = proof.latch;
        let header_index = blocks[&loop_.header];
        let latch_index = blocks[&latch_at];
        let preheader_index = blocks[&preheader];

        // Proof occurrences are snapshot-local, never `Op.id`, addresses, or
        // structural equality.  Resolve them before rebuilding the body.
        let phi = &body.blocks[proof.phi.block_index()].phis[proof.phi.phi_index()];
        let compare =
            &body.blocks[proof.compare.block_index()].ops[proof.compare.operation_index()];
        let branch = &body.blocks[proof.branch.block_index()].ops[proof.branch.operation_index()];
        let stepping = &body.blocks[replacement.stepping.block_index()].ops
            [replacement.stepping.operation_index()];
        let header = &body.blocks[header_index];
        let latch = &body.blocks[latch_index];
        let preheader_block = &body.blocks[preheader_index];
        let width = match &proof.counter.start {
            AffineOperand::Held(held) => held.width,
            AffineOperand::Const(constant) => constant.width,
        };

        let serial = all_values.iter().map(|value| value.id).max().unwrap_or(0) + 1;
        let variable = all_values
            .iter()
            .map(|value| value.variable)
            .max()
            .unwrap_or(0)
            + 1;
        let step_flags = Value {
            id: serial,
            at: stepping.at,
            flags: true,
            variable,
            version: 1,
        };
        let guard_flags = Value {
            id: serial + 1,
            at: preheader,
            flags: true,
            variable: variable + 1,
            version: 1,
        };
        let mut decrement = stepping.clone();
        decrement.name.clear();
        decrement.defines = stepping
            .defines
            .iter()
            .copied()
            .filter(|value| !value.flags)
            .chain(std::iter::once(step_flags))
            .collect();
        decrement.uses = vec![phi.result];
        decrement.source_backed = false;
        decrement.kind = Kind::Decrement;
        decrement.args = vec![Arg::Held(Held {
            value: phi.result,
            width,
        })];
        decrement.raised = None;
        decrement.symbol = Some(false);

        let mut guard_compare = compare.clone();
        guard_compare.at = preheader_block.ops.last().map_or(preheader, |op| op.at);
        guard_compare.defines = vec![guard_flags];
        guard_compare.uses = vec![bound.value];
        guard_compare.source_backed = false;
        guard_compare.args = vec![Arg::Held(*bound), Arg::Const(Const::new(0, width))];
        guard_compare.raised = None;
        guard_compare.absorbed.clear();
        guard_compare.symbol = Some(false);

        let mut guard_branch = branch.clone();
        guard_branch.at = guard_compare.at;
        guard_branch.name.clear();
        guard_branch.defines.clear();
        guard_branch.uses = vec![guard_flags];
        guard_branch.source_backed = false;
        guard_branch.test = Some(Kind::Eq);
        guard_branch.target = Some(proof.exit);
        guard_branch.raised = None;
        guard_branch.absorbed.clear();
        guard_branch.symbol = Some(false);

        let mut entry_ops = preheader_block.ops.clone();
        if let Some(last) = entry_ops.last_mut() {
            if last.kind == Kind::Jump {
                *last = mir::cleared(last);
            } else if last.kind == Kind::Branch {
                continue;
            }
        }
        entry_ops.extend([guard_compare, guard_branch]);

        let start = *phi
            .incoming
            .get(&preheader)
            .expect("counted proof has the preheader phi input");
        let start_definition = made.get(&start.id).copied();
        let start_is_private = start_definition.is_some()
            && !body
                .blocks
                .iter()
                .flat_map(|block| &block.ops)
                .any(|operation| operation.uses.contains(&start))
            && !occurrence::phis(body).any(|(other_occurrence, _, other)| {
                other_occurrence != proof.phi
                    && other.incoming.values().any(|value| *value == start)
            });
        if let Some(start_definition) = start_definition.filter(|_| start_is_private) {
            for (operation_index, operation) in entry_ops.iter_mut().enumerate() {
                if preheader_index == start_definition.block_index()
                    && operation_index == start_definition.operation_index()
                {
                    *operation = mir::cleared(operation);
                }
            }
        }

        let rewritten = body
            .blocks
            .iter()
            .enumerate()
            .map(|(block_index, block)| {
                let mut ops = block
                    .ops
                    .iter()
                    .enumerate()
                    .filter_map(|(operation_index, operation)| {
                        let is_stepping = block_index == replacement.stepping.block_index()
                            && operation_index == replacement.stepping.operation_index();
                        if is_stepping {
                            return None;
                        }
                        let mut operation = operation.clone();
                        let is_compare = block_index == proof.compare.block_index()
                            && operation_index == proof.compare.operation_index();
                        let is_branch = block_index == proof.branch.block_index()
                            && operation_index == proof.branch.operation_index();
                        let is_start_definition = start_definition.is_some_and(|definition| {
                            block_index == definition.block_index()
                                && operation_index == definition.operation_index()
                        });
                        if is_compare {
                            operation = mir::cleared(&operation);
                        } else if is_branch {
                            operation.name.clear();
                            operation.uses = vec![step_flags];
                            operation.source_backed = false;
                            operation.test = Some(Kind::Ne);
                            operation.target = Some(latch.at);
                            operation.raised = None;
                            operation.symbol = Some(false);
                        } else if start_is_private && is_start_definition {
                            operation = mir::cleared(&operation);
                        }
                        Some(operation)
                    })
                    .collect::<Vec<_>>();
                if block.at == latch_at {
                    let cut =
                        ops.len() - usize::from(ops.last().is_some_and(|op| op.kind == Kind::Jump));
                    ops.insert(cut, decrement.clone());
                }
                let phis = if block.at == header.at {
                    block
                        .phis
                        .iter()
                        .enumerate()
                        .map(|(phi_index, other)| {
                            if block_index == proof.phi.block_index()
                                && phi_index == proof.phi.phi_index()
                            {
                                Phi {
                                    result: other.result,
                                    incoming: OrderedMap::from_iter([
                                        (preheader, bound.value),
                                        (latch_at, replacement.update),
                                    ]),
                                }
                            } else {
                                other.clone()
                            }
                        })
                        .collect()
                } else {
                    block.phis.clone()
                };
                MirBlock {
                    at: block.at,
                    phis,
                    ops,
                    succ: block.succ.clone(),
                }
            })
            .collect::<Vec<_>>();
        let changed = MirBody {
            blocks: rewritten,
            ..body.clone()
        };
        // Python's `MirBody.block` is a first-match scan, deliberately
        // unlike its earlier last-wins address dictionary.
        let changed_header = changed
            .block(header.at)
            .expect("counted proof header remains in reconstructed body");
        let changed_latch = changed
            .block(latch.at)
            .expect("counted proof latch remains in reconstructed body");
        return counted_down(&at_body(
            &changed,
            &loop_,
            preheader,
            changed_header,
            changed_latch,
            &entry_ops,
            Some(&[changed_latch.at, proof.exit]),
        )?);
    }

    Ok(body.clone())
}

/// Enter every proven loop at its body, then merge its old test into the latch.
///
/// Direct port of `qbopt/optimize/rotate.py:entered`.  This is intentionally
/// the final composition: after rotation the loop no longer has the pretested
/// shape the central induction proofs consume.
pub(crate) fn entered(body: &MirBody) -> Result<MirBody, SubstitutionError> {
    cfg::merged(&rotated(&counted_down(body)?)?)
}

/// Rotate a loop whose initial test has already been proven to pass.
///
/// Direct port of `qbopt/optimize/rotate.py:rotated`.  This function only
/// consumes induction facts; it does not establish an independent legality
/// proof.
pub(crate) fn rotated(body: &MirBody) -> Result<MirBody, SubstitutionError> {
    // Python's address dictionary retains the last duplicate occurrence.
    let blocks = body
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.at, index))
        .collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(&body.blocks);

    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        if loop_.latches.len() != 1 {
            continue;
        }
        let Some(preheader) = transform::preheader(body, &loop_) else {
            continue;
        };
        let preheader_block = &body.blocks[blocks[&preheader]];
        if preheader_block.succ.as_slice() != [loop_.header] {
            continue;
        }
        let header = &body.blocks[blocks[&loop_.header]];
        let inside = header
            .succ
            .iter()
            .copied()
            .filter(|at| loop_.body.contains(at))
            .collect::<Vec<_>>();
        if header.succ.len() != 2 || inside.len() != 1 || inside[0] == header.at {
            continue;
        }
        let first = &body.blocks[blocks[&inside[0]]];
        if !first.phis.is_empty()
            || predecessors.get(&first.at) != Some(&BTreeSet::from([header.at]))
        {
            continue;
        }
        if header.ops.is_empty() || header.ops.last().is_none_or(|op| op.kind != Kind::Branch) {
            continue;
        }
        if !header.ops[..header.ops.len() - 1]
            .iter()
            .all(induction::test_only)
            || !induction::nonempty(body, &loop_)
        {
            continue;
        }

        let entry = &body.blocks[blocks[&preheader]];
        let mut ops = entry.ops.clone();
        if let Some(last) = ops.last_mut() {
            if last.kind == Kind::Branch {
                continue;
            }
            if last.kind == Kind::Jump {
                last.target = Some(first.at);
            } else {
                let mut jump = Op::new(last.at, OpCode::jump(), "", Vec::new(), Vec::new());
                jump.kind = Kind::Jump;
                jump.target = Some(first.at);
                jump.symbol = Some(false);
                ops.push(jump);
            }
        } else {
            let mut jump = Op::new(entry.at, OpCode::jump(), "", Vec::new(), Vec::new());
            jump.kind = Kind::Jump;
            jump.target = Some(first.at);
            jump.symbol = Some(false);
            ops.push(jump);
        }

        let stepped = step_test(body, &loop_, header);
        // Python's `MirBody.block` is a first-match scan after `_step_test`,
        // unlike the earlier last-wins address map.
        let stepped_header = stepped
            .block(header.at)
            .expect("rotation header remains in reconstructed body");
        let stepped_first = stepped
            .block(first.at)
            .expect("rotation first block remains in reconstructed body");
        return rotated(&at_body(
            &stepped,
            &loop_,
            preheader,
            stepped_header,
            stepped_first,
            &ops,
            None,
        )?);
    }

    Ok(body.clone())
}

/// Replace a zero comparison with the flags from its latch step.
///
/// Direct port of `qbopt/optimize/rotate.py:_step_test`.  The exact trip
/// proof stays in `analysis::induction`; this only makes the proven control
/// shape explicit in MIR.
fn step_test(body: &MirBody, loop_: &Loop, header: &MirBlock) -> MirBody {
    if loop_.latches.len() != 1
        || header.ops.is_empty()
        || header.ops.last().is_none_or(|op| op.kind != Kind::Branch)
    {
        return body.clone();
    }
    let latch_at = *loop_.latches.iter().next().expect("one latch exists");
    // Python `body.block`, not an address dictionary.
    let latch = body
        .block(latch_at)
        .expect("sole loop latch is a MIR block");
    let branch = header.ops.last().expect("checked nonempty");
    if !matches!(branch.test, Some(Kind::Eq | Kind::Ne)) {
        return body.clone();
    }

    // Both maps are Python's last-wins `{value.id: op ...}` relation.  The
    // occurrence map preserves `is` for the later reconstruction.
    let made = body
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter())
        .flat_map(|operation| {
            operation
                .defines
                .iter()
                .map(move |value| (value.id, operation))
        })
        .collect::<BTreeMap<_, _>>();
    let made_occurrences = occurrence::operations(body)
        .flat_map(|(occurrence, _, operation)| {
            operation
                .defines
                .iter()
                .map(move |value| (value.id, occurrence))
        })
        .collect::<BTreeMap<_, _>>();
    let mut readers = BTreeMap::<Value, Vec<occurrence::OpOccurrence>>::new();
    for (occurrence, _, operation) in occurrence::operations(body) {
        for value in &operation.defines {
            readers.entry(*value).or_default();
        }
        for value in &operation.uses {
            readers.entry(*value).or_default().push(occurrence);
        }
    }
    let facts = consts::known(body, None, None, None, None);
    let header_index = body
        .blocks
        .iter()
        .position(|block| std::ptr::eq(block, header))
        .expect("rotation header belongs to this MIR snapshot");

    for counter in induction::basics(body, loop_).values() {
        let Some(phi) = header
            .phis
            .iter()
            .find(|one| one.result.id == counter.value)
        else {
            continue;
        };
        let Some(update) = phi.incoming.get(&latch_at).copied() else {
            continue;
        };
        let width = match &counter.start {
            AffineOperand::Held(held) => held.width,
            AffineOperand::Const(constant) => constant.width,
        };
        let comparisons = header.ops[..header.ops.len() - 1]
            .iter()
            .enumerate()
            .filter(|(_, operation)| {
                induction::_counter_bound(operation, branch, counter, width, Some(&made))
                    == Some(Arg::Const(Const::new(0, width)))
            })
            .collect::<Vec<_>>();
        if comparisons.len() != 1 {
            continue;
        }
        let (compare_index, compare) = comparisons[0];
        let flags = compare
            .defines
            .iter()
            .copied()
            .filter(|value| value.flags)
            .collect::<Vec<_>>();
        // Python's `[branch]` comparison is dataclass structural equality,
        // not identity.  Keep the reader occurrence for the later `is` edit.
        if flags.len() != 1
            || !readers.get(&flags[0]).is_some_and(|users| {
                users.len() == 1
                    && body.blocks[users[0].block_index()].ops[users[0].operation_index()]
                        == *branch
            })
        {
            continue;
        }
        let Some(stepping) = made.get(&update.id).copied() else {
            continue;
        };
        let stepping_occurrence = made_occurrences
            .get(&update.id)
            .copied()
            .expect("last-wins stepping definition has an occurrence");
        let Some(stepped) = mir::stepping(stepping) else {
            continue;
        };
        if stepped.0
            != Arg::Held(Held {
                value: phi.result,
                width,
            })
            || stepping.results
                != vec![Arg::Held(Held {
                    value: update,
                    width,
                })]
            || !stepping.loads.is_empty()
            || !stepping.stores.is_empty()
            || stepping.barrier()
            || !stepping.merges.is_empty()
        {
            continue;
        }
        // Python list.index uses structural equality and selects the first
        // equal operation, even if `made` selected a later equal occurrence.
        let step_index = latch
            .ops
            .iter()
            .position(|operation| operation == stepping)
            .expect("proof-selected stepping operation belongs to the loop latch");
        if latch.ops[step_index + 1..]
            .iter()
            .any(|operation| !matches!(operation.kind, Kind::Nothing | Kind::Jump))
        {
            continue;
        }
        if stepping
            .defines
            .iter()
            .any(|value| value.flags && readers.get(value).is_some_and(|users| !users.is_empty()))
        {
            continue;
        }
        if induction::trip_count(body, loop_, &facts).is_none() {
            continue;
        }

        let serial = ssa::values(body).map(|value| value.id).max().unwrap_or(0) + 1;
        let variable = ssa::values(body)
            .map(|value| value.variable)
            .max()
            .unwrap_or(0)
            + 1;
        let step_flags = Value {
            id: serial,
            at: stepping.at,
            flags: true,
            variable,
            version: 1,
        };
        let blocks = body
            .blocks
            .iter()
            .enumerate()
            .map(|(block_index, block)| MirBlock {
                at: block.at,
                phis: block.phis.clone(),
                ops: block
                    .ops
                    .iter()
                    .enumerate()
                    .map(|(operation_index, operation)| {
                        if block_index == stepping_occurrence.block_index()
                            && operation_index == stepping_occurrence.operation_index()
                        {
                            let mut rewritten = operation.clone();
                            rewritten.defines.push(step_flags);
                            rewritten.source_backed = false;
                            rewritten.raised = None;
                            rewritten
                        } else if block_index == header_index && operation_index == compare_index {
                            mir::cleared(operation)
                        } else if block_index == header_index
                            && operation_index == header.ops.len() - 1
                        {
                            let mut rewritten = operation.clone();
                            rewritten.uses = vec![step_flags];
                            rewritten.source_backed = false;
                            rewritten.raised = None;
                            rewritten
                        } else {
                            operation.clone()
                        }
                    })
                    .collect(),
                succ: block.succ.clone(),
            })
            .collect();
        return MirBody {
            blocks,
            ..body.clone()
        };
    }

    body.clone()
}

/// Enter `loop` at `first`, moving its header phis there by hand.
///
/// Direct port of `qbopt/optimize/rotate.py:at_body`.  Python raises from
/// `ssa.substituted` on a cyclic map; Rust makes that mechanical difference
/// explicit through [`SubstitutionError`] rather than silently retaining an
/// old operation.
pub(crate) fn at_body(
    body: &MirBody,
    loop_: &Loop,
    preheader: i64,
    header: &MirBlock,
    first: &MirBlock,
    ops: &[Op],
    entry_succ: Option<&[i64]>,
) -> Result<MirBody, SubstitutionError> {
    let latch = *loop_
        .latches
        .iter()
        .next()
        .expect("at_body requires one latch");
    let serial = ssa::values(body)
        .chain(
            ops.iter()
                .flat_map(|op| op.defines.iter().chain(&op.uses).chain(&op.exits).copied()),
        )
        .map(|value| value.id)
        .max()
        .expect("at_body requires a value")
        + 1;
    let moved = header
        .phis
        .iter()
        .enumerate()
        .map(|(index, phi)| {
            (
                phi.result.id,
                Value {
                    id: serial + index as u32,
                    at: first.at,
                    ..phi.result
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    let latest = |value: Value| moved.get(&value.id).copied().unwrap_or(value);
    let ending = header
        .phis
        .iter()
        .map(|phi| {
            (
                phi.result.id,
                latest(
                    *phi.incoming
                        .get(&latch)
                        .expect("header phi has a latch input"),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let initial = header
        .phis
        .iter()
        .map(|phi| {
            (
                phi.result.id,
                *phi.incoming
                    .get(&preheader)
                    .expect("header phi has a preheader input"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let inside = loop_
        .body
        .iter()
        .copied()
        .filter(|at| *at != header.at)
        .collect::<std::collections::BTreeSet<_>>();
    let entry = header
        .phis
        .iter()
        .map(|phi| Phi {
            result: moved[&phi.result.id],
            incoming: OrderedMap::from_iter([
                (
                    preheader,
                    *phi.incoming
                        .get(&preheader)
                        .expect("header phi has a preheader input"),
                ),
                (header.at, ending[&phi.result.id]),
            ]),
        })
        .collect::<Vec<_>>();

    let mut counts = body
        .loop_trip_counts
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();
    if let Some(count) = counts.remove(&header.at) {
        if counts
            .get(&first.at)
            .is_none_or(|previous| *previous == count)
        {
            counts.insert(first.at, count);
        }
    }

    let mut integer_ranges = body.integer_ranges.clone();
    for phi in &header.phis {
        if let Some(interval) = integer_ranges.remove(&phi.result) {
            integer_ranges.insert(moved[&phi.result.id], interval);
        }
    }

    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let swap = if inside.contains(&block.at) {
                &moved
            } else {
                &ending
            };
            let phis = block
                .phis
                .iter()
                .map(|phi| {
                    let mut incoming = phi
                        .incoming
                        .iter()
                        .map(|(at, value)| {
                            (
                                *at,
                                if inside.contains(at) {
                                    moved.get(&value.id).copied().unwrap_or(*value)
                                } else {
                                    ending.get(&value.id).copied().unwrap_or(*value)
                                },
                            )
                        })
                        .collect::<OrderedMap<_, _>>();
                    if entry_succ.is_some_and(|successors| successors.contains(&block.at))
                        && block.at != first.at
                    {
                        if let Some(zero) = phi.incoming.get(&header.at) {
                            incoming
                                .insert(preheader, initial.get(&zero.id).copied().unwrap_or(*zero));
                        }
                    }
                    Phi {
                        result: phi.result,
                        incoming,
                    }
                })
                .collect::<Vec<_>>();
            let rewritten = MirBlock {
                at: block.at,
                phis,
                ops: block
                    .ops
                    .iter()
                    .map(|op| swapped(op, swap))
                    .collect::<Result<_, _>>()?,
                succ: block.succ.clone(),
            };
            if block.at == preheader {
                Ok(MirBlock {
                    ops: ops.to_vec(),
                    succ: entry_succ
                        .filter(|successors| !successors.is_empty())
                        .map_or_else(|| vec![first.at], |successors| successors.to_vec()),
                    ..rewritten
                })
            } else if block.at == header.at {
                Ok(MirBlock {
                    phis: Vec::new(),
                    ..rewritten
                })
            } else if block.at == first.at {
                Ok(MirBlock {
                    phis: entry.clone(),
                    ..rewritten
                })
            } else {
                Ok(rewritten)
            }
        })
        .collect::<Result<Vec<_>, SubstitutionError>>()?;

    Ok(MirBody {
        blocks,
        integer_ranges,
        loop_trip_counts: counts.into_iter().collect(),
        ..body.clone()
    })
}

/// Apply one non-chained id substitution to an operation.
///
/// Direct port of `qbopt/optimize/rotate.py:_swapped`.
fn swapped(op: &Op, swap: &BTreeMap<u32, Value>) -> Result<Op, SubstitutionError> {
    if swap.is_empty() || !op.uses.iter().any(|value| swap.contains_key(&value.id)) {
        return Ok(op.clone());
    }
    let one_step = swap
        .iter()
        .filter(|(source, target)| !swap.contains_key(&target.id) || target.id == **source)
        .map(|(source, target)| (*source, *target))
        .collect::<BTreeMap<_, _>>();
    ssa::substituted(op, &one_step)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use num_bigint::BigInt;

    use super::{at_body, entered, rotated, swapped};
    use crate::analysis::loops;
    use crate::analysis::loops::Loop;
    use crate::model::mir::{
        Arg, Cell, Const, Held, IntegerRange, Kind, MemRef, MirBlock, MirBody, Op, Phi, Value,
    };
    use crate::objectfile::module::Space;

    fn value(id: u32, at: i64) -> Value {
        Value::new(id, at)
    }

    fn operation(at: i64, kind: Kind, defines: Vec<Value>, uses: Vec<Value>) -> Op {
        let mut op = Op::new(at, None, "", defines, uses);
        op.kind = kind;
        op
    }

    fn phi(result: Value, incoming: &[(i64, Value)]) -> Phi {
        Phi {
            result,
            incoming: incoming.iter().copied().collect(),
        }
    }

    /// Direct Rust form of `tests/test_countdown.py:counted_loop`.
    fn counted_loop(observed: bool) -> MirBody {
        let seed = Value {
            variable: 1,
            version: 1,
            ..value(1, 0)
        };
        let bound = Value {
            variable: 2,
            version: 1,
            ..value(2, 0)
        };
        let counter = Value {
            variable: 1,
            version: 2,
            ..value(3, 1)
        };
        let following = Value {
            variable: 1,
            version: 3,
            ..value(4, 2)
        };
        let flags = Value {
            flags: true,
            variable: 3,
            version: 1,
            ..value(5, 1)
        };
        let mut source = MemRef::new(None, 2);
        source.space = Some(Space::Frame);
        let mut sink = MemRef::new(None, 2);
        sink.space = Some(Space::Segment);

        let mut initialize = operation(0, Kind::Copy, vec![seed], vec![]);
        initialize.args = vec![Arg::Const(Const::new(0, 2))];
        initialize.results = vec![Arg::Held(Held {
            value: seed,
            width: 2,
        })];
        let mut load = operation(0, Kind::Load, vec![bound], vec![]);
        load.loads = vec![source.clone()];
        load.args = vec![Arg::Cell(Cell {
            r#ref: source.clone(),
        })];
        load.results = vec![Arg::Held(Held {
            value: bound,
            width: 2,
        })];
        let mut compare = operation(1, Kind::Sub, vec![flags], vec![counter, bound]);
        compare.args = vec![
            Arg::Held(Held {
                value: counter,
                width: 2,
            }),
            Arg::Held(Held {
                value: bound,
                width: 2,
            }),
        ];
        let mut branch = operation(1, Kind::Branch, vec![], vec![flags]);
        branch.test = Some(Kind::AboveEq);
        branch.target = Some(3);
        let mut store = operation(2, Kind::Store, vec![], vec![counter]);
        store.stores = vec![sink.clone()];
        store.args = vec![Arg::Held(Held {
            value: counter,
            width: 2,
        })];
        store.results = vec![Arg::Cell(Cell { r#ref: sink })];
        let mut increment = operation(2, Kind::Increment, vec![following], vec![counter]);
        increment.args = vec![Arg::Held(Held {
            value: counter,
            width: 2,
        })];
        increment.results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        let mut jump = operation(2, Kind::Jump, vec![], vec![]);
        jump.target = Some(1);
        let returned = operation(3, Kind::Return, vec![], vec![]);
        let mut body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![initialize, load], vec![1]),
                MirBlock::new(
                    1,
                    vec![phi(counter, &[(0, seed), (2, following)])],
                    vec![compare, branch],
                    vec![2, 3],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    if observed {
                        vec![store, increment, jump]
                    } else {
                        vec![increment, jump]
                    },
                    vec![1],
                ),
                MirBlock::new(3, vec![], vec![returned], vec![]),
            ],
        );
        body.sealed = true;
        body
    }

    #[test]
    fn dead_dynamic_counter_counts_down_on_step_flags_after_zero_trip_guard() {
        // Direct port of
        // `tests/test_countdown.py:test_dead_dynamic_counter_counts_down_on_the_step_flags_after_a_zero_trip_guard`.
        // C floats retained add/cmp/jb through a ten-trip hot path until the
        // dynamic zero-trip guard made the countdown form exact.
        let body = entered(&counted_loop(false)).unwrap();
        let loops = loops::loops(&body.blocks, Some(body.entry));
        let [loop_] = loops.as_slice() else {
            panic!("the transformed body retains exactly one natural loop");
        };
        let decrements = body
            .blocks
            .iter()
            .filter(|block| loop_.body.contains(&block.at))
            .flat_map(|block| &block.ops)
            .filter(|operation| operation.kind == Kind::Decrement)
            .collect::<Vec<_>>();
        assert_eq!(decrements.len(), 1);
        let flags = decrements[0]
            .defines
            .iter()
            .copied()
            .filter(|value| value.flags)
            .collect::<BTreeSet<_>>();
        let backedges = body
            .blocks
            .iter()
            .filter(|block| loop_.body.contains(&block.at))
            .flat_map(|block| &block.ops)
            .filter(|operation| {
                operation.kind == Kind::Branch
                    && operation
                        .target
                        .is_some_and(|target| loop_.body.contains(&target))
            })
            .collect::<Vec<_>>();
        assert_eq!(backedges.len(), 1);
        assert_eq!(backedges[0].test, Some(Kind::Ne));
        assert!(backedges[0].uses.iter().any(|value| flags.contains(value)));

        let predecessors = loops::predecessors(&body.blocks);
        let entries = predecessors[&loop_.header]
            .iter()
            .copied()
            .filter(|at| !loop_.body.contains(at))
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        let guard = body
            .block(entries[0])
            .expect("guard predecessor is a block");
        assert_eq!(guard.succ.len(), 2);
        assert!(guard.succ.iter().any(|at| !loop_.body.contains(at)));
        assert_eq!(
            guard.ops.last().map(|operation| operation.kind),
            Some(Kind::Branch)
        );
        assert_eq!(
            guard.ops.last().and_then(|operation| operation.test),
            Some(Kind::Eq)
        );
    }

    #[test]
    fn countdown_refuses_an_observed_source_counter() {
        // Direct port of
        // `tests/test_countdown.py:test_countdown_refuses_an_observed_source_counter`.
        // Replacing an index the body stores would change the program.
        let body = counted_loop(true);
        assert_eq!(entered(&body).unwrap(), body);
    }

    /// Direct port of the pretested `FOR` shape covered by
    /// `tests/test_rotate.py:test_entering_mains_first_loop_at_its_body_keeps_the_code_after_it`.
    #[test]
    fn rotated_enters_a_proven_pretested_loop_at_its_body_and_moves_header_phis() {
        let initial = Value {
            variable: 1,
            version: 1,
            ..value(1, 0)
        };
        let counter = Value {
            variable: 1,
            version: 2,
            ..value(2, 1)
        };
        let next = Value {
            variable: 1,
            version: 3,
            ..value(3, 2)
        };
        let flags = Value {
            flags: true,
            variable: 2,
            version: 1,
            ..value(4, 1)
        };
        let mut compare = operation(1, Kind::Sub, vec![flags], vec![counter]);
        compare.args = vec![
            Arg::Held(Held {
                value: counter,
                width: 2,
            }),
            Arg::Const(Const::new(4, 2)),
        ];
        let mut branch = operation(1, Kind::Branch, Vec::new(), vec![flags]);
        branch.test = Some(Kind::Below);
        branch.target = Some(3);
        let mut update = operation(2, Kind::Increment, vec![next], vec![counter]);
        update.args = vec![Arg::Held(Held {
            value: counter,
            width: 2,
        })];
        update.results = vec![Arg::Held(Held {
            value: next,
            width: 2,
        })];
        let mut backedge = operation(2, Kind::Jump, Vec::new(), Vec::new());
        backedge.target = Some(1);
        let body = MirBody {
            loop_trip_counts: vec![(1, 4)],
            ..MirBody::new(
                0,
                vec![
                    MirBlock::new(0, vec![], vec![], vec![1]),
                    MirBlock::new(
                        1,
                        vec![phi(counter, &[(0, initial), (2, next)])],
                        vec![compare, branch],
                        vec![2, 3],
                    ),
                    MirBlock::new(2, vec![], vec![update, backedge], vec![1]),
                    MirBlock::new(
                        3,
                        vec![],
                        vec![operation(3, Kind::Return, vec![], vec![])],
                        vec![],
                    ),
                ],
            )
        };

        let changed = rotated(&body).unwrap();
        assert_eq!(changed.block(0).expect("preheader").succ, vec![2]);
        assert_eq!(changed.block(1).expect("old header").phis, Vec::new());
        let entered = changed.block(2).expect("body entry");
        assert_eq!(entered.phis.len(), 1);
        assert_eq!(entered.phis[0].incoming.get(&0), Some(&initial));
        assert_eq!(entered.phis[0].incoming.get(&1), Some(&next));
        assert!(
            changed
                .block(0)
                .expect("preheader")
                .ops
                .last()
                .is_some_and(
                    |operation| operation.kind == Kind::Jump && operation.target == Some(2)
                )
        );
    }

    /// A zero-ending countdown needs no second compare after rotation: the
    /// latch decrement's flags are exactly the branch question.
    #[test]
    fn rotated_reuses_a_zero_ending_step_flags_for_the_latch_test() {
        let seed = Value {
            variable: 1,
            version: 1,
            ..value(1, 0)
        };
        let counter = Value {
            variable: 1,
            version: 2,
            ..value(2, 1)
        };
        let next = Value {
            variable: 1,
            version: 3,
            ..value(3, 2)
        };
        let compare_flags = Value {
            flags: true,
            variable: 2,
            version: 1,
            ..value(4, 1)
        };
        let mut seed_op = operation(0, Kind::Copy, vec![seed], vec![]);
        seed_op.args = vec![Arg::Const(Const::new(3, 2))];
        seed_op.results = vec![Arg::Held(Held {
            value: seed,
            width: 2,
        })];
        let mut compare = operation(1, Kind::Sub, vec![compare_flags], vec![counter]);
        compare.args = vec![
            Arg::Held(Held {
                value: counter,
                width: 2,
            }),
            Arg::Const(Const::new(0, 2)),
        ];
        let mut branch = operation(1, Kind::Branch, Vec::new(), vec![compare_flags]);
        branch.test = Some(Kind::Ne);
        branch.target = Some(2);
        let mut decrement = operation(2, Kind::Decrement, vec![next], vec![counter]);
        decrement.args = vec![Arg::Held(Held {
            value: counter,
            width: 2,
        })];
        decrement.results = vec![Arg::Held(Held {
            value: next,
            width: 2,
        })];
        let mut backedge = operation(2, Kind::Jump, Vec::new(), Vec::new());
        backedge.target = Some(1);
        let body = MirBody {
            loop_trip_counts: vec![(1, 3)],
            ..MirBody::new(
                0,
                vec![
                    MirBlock::new(0, vec![], vec![seed_op], vec![1]),
                    MirBlock::new(
                        1,
                        vec![phi(counter, &[(0, seed), (2, next)])],
                        vec![compare, branch],
                        vec![2, 3],
                    ),
                    MirBlock::new(2, vec![], vec![decrement, backedge], vec![1]),
                    MirBlock::new(
                        3,
                        vec![],
                        vec![operation(3, Kind::Return, vec![], vec![])],
                        vec![],
                    ),
                ],
            )
        };

        let changed = rotated(&body).unwrap();
        assert_eq!(changed.block(0).expect("preheader").succ, vec![2]);
        let old_test = changed.block(1).expect("old test");
        assert_eq!(old_test.ops[0].kind, Kind::Nothing);
        let branch = old_test.ops.last().expect("latch test remains");
        let decrement = changed
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .find(|operation| operation.kind == Kind::Decrement)
            .expect("decrement remains");
        let step_flags = decrement
            .defines
            .iter()
            .copied()
            .find(|value| value.flags)
            .expect("step acquires its flags");
        assert_eq!(branch.uses, vec![step_flags]);
    }

    #[test]
    fn rotated_refuses_unknown_trip_counts_and_non_test_header_work() {
        let body = counted_loop(false);
        assert_eq!(rotated(&body).unwrap(), body);

        let mut with_work = counted_loop(false);
        with_work.loop_trip_counts = vec![(1, 2)];
        with_work.blocks[1]
            .ops
            .insert(0, operation(1, Kind::Copy, vec![], vec![]));
        assert_eq!(rotated(&with_work).unwrap(), with_work);

        let mut branch_ended_entry = counted_loop(false);
        branch_ended_entry.loop_trip_counts = vec![(1, 2)];
        let mut branch = operation(0, Kind::Branch, Vec::new(), Vec::new());
        branch.target = Some(1);
        branch_ended_entry.blocks[0].ops.push(branch);
        assert_eq!(rotated(&branch_ended_entry).unwrap(), branch_ended_entry);
    }

    /// Header phi rotation must distinguish split values of one variable:
    /// `tests/test_rotate.py` regressed when a post-loop call answer was
    /// renamed to the loop counter and the exit disappeared.
    #[test]
    fn at_body_moves_phis_and_substitutes_one_step() {
        let initial = Value {
            variable: 9,
            version: 1,
            ..value(1, 0)
        };
        let counter = Value {
            variable: 9,
            version: 2,
            ..value(2, 1)
        };
        let next = Value {
            variable: 9,
            version: 3,
            ..value(3, 2)
        };
        let same_variable_but_not_phi = Value {
            variable: 9,
            version: 4,
            ..value(4, 3)
        };
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(
                    0,
                    vec![],
                    vec![operation(0, Kind::Jump, vec![], vec![])],
                    vec![1],
                ),
                MirBlock::new(
                    1,
                    vec![phi(counter, &[(0, initial), (3, next)])],
                    vec![operation(1, Kind::Branch, vec![], vec![counter])],
                    vec![2, 4],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![operation(2, Kind::Add, vec![next], vec![counter])],
                    vec![3],
                ),
                MirBlock::new(
                    3,
                    vec![],
                    vec![operation(3, Kind::Jump, vec![], vec![])],
                    vec![1],
                ),
                MirBlock::new(
                    4,
                    vec![],
                    vec![operation(
                        4,
                        Kind::Return,
                        vec![],
                        vec![next, same_variable_but_not_phi],
                    )],
                    vec![],
                ),
            ],
        );
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::from([3]),
            body: BTreeSet::from([1, 2, 3]),
        };
        let changed = at_body(
            &body,
            &loop_,
            0,
            &body.blocks[1],
            &body.blocks[2],
            &body.blocks[0].ops,
            None,
        )
        .unwrap();

        let moved = Value {
            id: 5,
            at: 2,
            ..counter
        };
        assert!(changed.blocks[1].phis.is_empty());
        assert_eq!(
            changed.blocks[2].phis,
            vec![phi(moved, &[(0, initial), (1, next)])]
        );
        assert_eq!(changed.blocks[2].ops[0].uses, vec![moved]);
        assert_eq!(
            changed.blocks[4].ops[0].uses,
            vec![next, same_variable_but_not_phi]
        );
        assert_eq!(changed.blocks[0].succ, vec![2]);
    }

    #[test]
    fn swapped_filters_chains_and_only_starts_from_operation_uses() {
        let first = value(1, 0);
        let middle = value(2, 1);
        let last = value(3, 2);
        let mut memory = MemRef::new(None, 2);
        memory.base = Some(first);
        let mut only_memory = operation(0, Kind::Load, vec![], vec![]);
        only_memory.loads = vec![memory.clone()];
        assert_eq!(
            swapped(&only_memory, &BTreeMap::from([(first.id, middle)])).unwrap(),
            only_memory
        );
        let mut uses = operation(0, Kind::Load, vec![], vec![first]);
        uses.loads = vec![memory];
        let changed = swapped(
            &uses,
            &BTreeMap::from([(first.id, middle), (middle.id, last)]),
        )
        .unwrap();
        assert_eq!(changed.uses, vec![first]);
        assert_eq!(changed.loads[0].base, Some(first));
    }

    #[test]
    fn at_body_allocates_after_supplied_operations_and_repairs_zero_trip_exit_facts() {
        let initial = value(1, 0);
        let counter = value(2, 1);
        let next = value(3, 2);
        let guard = value(40, 0);
        let supplied_use = value(41, 0);
        let supplied_exit = value(42, 0);
        let exit_value = value(5, 4);
        let exit_phi = phi(exit_value, &[(1, counter), (8, value(6, 8))]);
        let mut body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![1]),
                MirBlock::new(
                    1,
                    vec![phi(counter, &[(0, initial), (3, next)])],
                    vec![],
                    vec![2, 4],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![operation(2, Kind::Add, vec![next], vec![counter])],
                    vec![3],
                ),
                MirBlock::new(3, vec![], vec![], vec![1]),
                MirBlock::new(4, vec![exit_phi], vec![], vec![]),
            ],
        );
        let range = IntegerRange::new(BigInt::from(0), BigInt::from(9), 2);
        body.integer_ranges.insert(counter, range.clone());
        body.loop_trip_counts = vec![(1, 7), (1, 8), (2, 99), (9, 1)];
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::from([3]),
            body: BTreeSet::from([1, 2, 3]),
        };
        let mut guard_op = operation(0, Kind::Branch, vec![guard], vec![supplied_use]);
        guard_op.exits = vec![supplied_exit];
        let changed = at_body(
            &body,
            &loop_,
            0,
            &body.blocks[1],
            &body.blocks[2],
            &[guard_op.clone()],
            Some(&[3, 4]),
        )
        .unwrap();

        let moved = Value {
            id: 43,
            at: 2,
            ..counter
        };
        assert_eq!(changed.blocks[2].phis[0].result, moved);
        assert_eq!(changed.blocks[0].ops, vec![guard_op.clone()]);
        assert_eq!(changed.blocks[0].succ, vec![3, 4]);
        assert_eq!(changed.blocks[4].phis[0].incoming.get(&0), Some(&initial));
        assert_eq!(changed.integer_ranges.get(&counter), None);
        assert_eq!(changed.integer_ranges.get(&moved), Some(&range));
        assert_eq!(changed.loop_trip_counts, vec![(2, 99), (9, 1)]);

        let mut no_collision = body.clone();
        no_collision.loop_trip_counts = vec![(1, 7), (1, 8), (9, 1)];
        let transferred = at_body(
            &no_collision,
            &loop_,
            0,
            &no_collision.blocks[1],
            &no_collision.blocks[2],
            &[guard_op],
            Some(&[3, 4]),
        )
        .unwrap();
        assert_eq!(transferred.loop_trip_counts, vec![(2, 8), (9, 1)]);
    }

    #[test]
    fn at_body_rebuilds_every_duplicate_address_block_in_source_order() {
        let initial = value(1, 0);
        let counter = value(2, 1);
        let next = value(3, 2);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![1]),
                MirBlock::new(
                    1,
                    vec![phi(counter, &[(0, initial), (3, next)])],
                    vec![],
                    vec![2],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![operation(2, Kind::Add, vec![next], vec![counter])],
                    vec![3],
                ),
                MirBlock::new(3, vec![], vec![], vec![1]),
                MirBlock::new(
                    2,
                    vec![],
                    vec![operation(20, Kind::Return, vec![], vec![counter])],
                    vec![],
                ),
            ],
        );
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::from([3]),
            body: BTreeSet::from([1, 2, 3]),
        };
        let changed = at_body(
            &body,
            &loop_,
            0,
            &body.blocks[1],
            &body.blocks[2],
            &[],
            None,
        )
        .unwrap();
        assert_eq!(changed.blocks.len(), 5);
        assert_eq!(changed.blocks[2].phis.len(), 1);
        assert_eq!(changed.blocks[4].phis.len(), 1);
        assert_eq!(
            changed.blocks[2].ops[0].uses[0].id,
            changed.blocks[2].phis[0].result.id
        );
        assert_eq!(
            changed.blocks[4].ops[0].uses[0].id,
            changed.blocks[4].phis[0].result.id
        );
    }
}
