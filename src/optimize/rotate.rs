//! Enter a loop proven to run at least once at its body, not at its test.
//!
//! BC writes `FOR` as `jmp test; body: ...; test: cmp; jle body`. Where the
//! first test is proven to pass the entry jump goes straight to the body, and
//! the test is then reached only from the latch, which it can merge into.
//!
//! Port of `qbopt/optimize/rotate.py`. Python's `op is x` is an
//! `occurrence` (block index, op index) comparison; an exception from
//! `ssa.substituted` is a `SubstitutionError`.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::loops::{self, Loop};
use crate::analysis::ssa::SubstitutionError;
use crate::analysis::{consts, induction, occurrence, ssa};
use crate::model::mir::{
    self, Arg, Const, Held, Kind, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value,
};

use super::counting::{self, Seeds};
use super::{cfg, transform};

/// Every proven loop entered at its body, each test merged into its latch.
///
/// After the passes, not among them: a rotated loop is no longer the
/// pretested shape the counted-loop analyses read, and peeling and
/// unrolling it in a later round found no loop to work on.
pub(crate) fn entered(body: &Rc<MirBody>) -> Result<Rc<MirBody>, SubstitutionError> {
    cfg::merged(&rotated(&_counted_down(body)?)?)
}

/// Rotate a dead counted counter into a guarded countdown.
///
/// A dynamic bound cannot prove that the loop is entered, so the ordinary
/// rotation below correctly leaves its initial test in place.  If the
/// induction value itself is otherwise dead, its only useful meaning is the
/// number of trips remaining:
///
///     i = start; while (i < n) { body; ++i; }
///
/// becomes a zero-trip guard followed by `--trips` and a branch on that
/// operation's own flags.  This is an induction-variable formula choice,
/// not a peephole: the guard is what makes a zero count exact, and refusing
/// an observed counter is what makes replacing its values sound.
///
/// The first implementation deliberately takes the canonical one-body-block
/// form produced by loop simplification.  More involved loops remain on the
/// original representation rather than acquiring a partially repaired CFG.
pub(crate) fn _counted_down(body: &Rc<MirBody>) -> Result<Rc<MirBody>, SubstitutionError> {
    let blocks = body
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.at, index))
        .collect::<BTreeMap<_, _>>();
    let facts = consts::known(body, None, None, None, None);
    let made = occurrence::operations(body)
        .flat_map(|(occurrence, _, op)| op.defines.iter().map(move |value| (value.id, occurrence)))
        .collect::<BTreeMap<_, _>>();
    let all_values = ssa::values(body).collect::<Vec<_>>();

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
        let (preheader, latch_at) = (proof.preheader, proof.latch);
        let (header, latch) = (
            &body.blocks[blocks[&loop_.header]],
            &body.blocks[blocks[&latch_at]],
        );
        let counter = &proof.counter;
        let phi = &body.blocks[proof.phi.block_index()].phis[proof.phi.phi_index()];
        let width = counter.start.width();
        let (update, stepping_at) = (replacement.update, replacement.stepping);
        let stepping = &body.blocks[stepping_at.block_index()].ops[stepping_at.operation_index()];
        let preheader_block = &body.blocks[blocks[&preheader]];
        let at = preheader_block.ops.last().map_or(preheader, |op| op.at);
        let mut seeds = Seeds {
            serial: all_values.iter().map(|value| value.id).max().unwrap_or(0) + 1,
            variable: all_values.iter().map(|value| value.variable).max().unwrap_or(0) + 1,
            at,
            width,
            ops: Vec::new(),
            facts: &facts,
        };
        let count = induction::trips(proof, &mut |kind, args| seeds.computed(kind, args));
        // A constant count is handled more profitably by the ordinary
        // finite-domain induction transforms.  This rewrite exists for a
        // symbolic value which may be zero at run time.
        let induction::AffineOperand::Held(count) = count else {
            continue;
        };
        let exits = counting::leaving(body, &replacement, &mut seeds);

        let step_flags = Value {
            id: seeds.serial,
            at: stepping.at,
            flags: true,
            variable: seeds.variable,
            version: 1,
        };
        let guard_flags = Value {
            id: seeds.serial + 1,
            at: preheader,
            flags: true,
            variable: seeds.variable + 1,
            version: 1,
        };
        let decrement = Op {
            name: String::new(),
            defines: stepping
                .defines
                .iter()
                .copied()
                .filter(|value| !value.flags)
                .chain([step_flags])
                .collect(),
            uses: vec![phi.result],
            source_backed: false,
            kind: Kind::Decrement,
            args: vec![Arg::Held(Held {
                value: phi.result,
                width,
            })],
            raised: None,
            symbol: Some(false),
            ..stepping.clone()
        };
        let (guard_compare, guard_branch) = counting::skip_guard(body, proof, at, guard_flags);
        let mut entry_ops = preheader_block.ops.clone();
        if entry_ops.last().is_some_and(|op| op.kind == Kind::Jump) {
            let last = entry_ops.len() - 1;
            entry_ops[last] = mir::cleared(&entry_ops[last]);
        } else if entry_ops.last().is_some_and(|op| op.kind == Kind::Branch) {
            continue;
        }
        let start = phi.incoming.get(&preheader).copied().expect("KeyError");
        let start_is_read_by_the_guard =
            seeds.ops.iter().chain([&guard_compare]).any(|op| op.uses.contains(&start));
        entry_ops.extend(seeds.ops);
        entry_ops.extend([guard_compare, guard_branch]);

        let start_definition = made.get(&start.id).copied();
        let start_is_private = start_definition.is_some()
            && !body
                .blocks
                .iter()
                .flat_map(|block| &block.ops)
                .any(|op| op.uses.contains(&start))
            && !start_is_read_by_the_guard
            && exits.is_empty()
            && !occurrence::phis(body).any(|(other_at, _, other)| {
                other_at != proof.phi && other.incoming.values().any(|value| *value == start)
            });
        let is_start_definition = |block_index: usize, op_index: usize| {
            start_definition.is_some_and(|definition| {
                definition.block_index() == block_index && definition.operation_index() == op_index
            })
        };
        if start_is_private {
            // `entry_ops` holds the preheader's own op objects, then the
            // guard ops, which are never the start definition.
            let preheader_index = blocks[&preheader];
            entry_ops = entry_ops
                .iter()
                .enumerate()
                .map(|(index, op)| {
                    if is_start_definition(preheader_index, index) {
                        mir::cleared(op)
                    } else {
                        op.clone()
                    }
                })
                .collect();
        }
        let mut rewritten = Vec::new();
        for (block_index, block) in body.blocks.iter().enumerate() {
            let mut ops = Vec::new();
            for (op_index, op) in block.ops.iter().enumerate() {
                let is = |at: occurrence::OpOccurrence| {
                    at.block_index() == block_index && at.operation_index() == op_index
                };
                let op = if is(stepping_at) {
                    // Canonicalize the control update after every data
                    // recurrence.  The rotated branch consumes its flags;
                    // leaving a strength-reduced address update after it
                    // would silently make those flags describe the address.
                    continue;
                } else if is(proof.compare) {
                    mir::cleared(op)
                } else if is(proof.branch) {
                    Op {
                        name: String::new(),
                        uses: vec![step_flags],
                        source_backed: false,
                        test: Some(Kind::Ne),
                        target: Some(latch.at),
                        raised: None,
                        symbol: Some(false),
                        ..op.clone()
                    }
                } else if start_is_private && is_start_definition(block_index, op_index) {
                    mir::cleared(op)
                } else {
                    op.clone()
                };
                ops.push(op);
            }
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
                                incoming: OrderedMap::from_iter([
                                    (preheader, count.value),
                                    (latch_at, update),
                                ]),
                                ..other.clone()
                            }
                        } else {
                            other.clone()
                        }
                    })
                    .collect()
            } else {
                block
                    .phis
                    .iter()
                    .enumerate()
                    .map(|(phi_index, other)| {
                        exits
                            .iter()
                            .find(|(at, _)| at.block_index() == block_index && at.phi_index() == phi_index)
                            .map_or_else(|| other.clone(), |(_, exit)| exit.clone())
                    })
                    .collect()
            };
            rewritten.push(MirBlock {
                ops,
                phis,
                ..block.clone()
            });
        }
        let changed = MirBody {
            blocks: rewritten,
            ..MirBody::clone(body)
        };
        let changed_header = changed.block(header.at).expect("header is a block");
        let changed_latch = changed.block(latch.at).expect("latch is a block");
        return _counted_down(&Rc::new(at_body(
            &changed,
            &loop_,
            preheader,
            changed_header,
            changed_latch,
            &entry_ops,
            Some(&[changed_latch.at, proof.exit]),
        )?));
    }
    Ok(body.clone())
}

pub(crate) fn rotated(body: &Rc<MirBody>) -> Result<Rc<MirBody>, SubstitutionError> {
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
        let Some(preheader) = transform::_preheader(body, &loop_) else {
            continue;
        };
        if body.blocks[blocks[&preheader]].succ != [loop_.header] {
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
            || predecessors.get(&first.at).cloned().unwrap_or_default()
                != BTreeSet::from([header.at])
        {
            continue;
        }
        if header.ops.last().is_none_or(|op| op.kind != Kind::Branch) {
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
        if ops.last().is_some_and(|op| op.kind == Kind::Branch) {
            continue;
        }
        if ops.last().is_some_and(|op| op.kind == Kind::Jump) {
            let last = ops.len() - 1;
            ops[last] = Op {
                target: Some(first.at),
                ..ops[last].clone()
            };
        } else {
            let at = ops.last().map_or(entry.at, |op| op.at);
            ops.push(Op {
                kind: Kind::Jump,
                target: Some(first.at),
                symbol: Some(false),
                ..Op::new(at, OpCode::jump(), "", Vec::new(), Vec::new())
            });
        }
        let body = _step_test(body, &loop_, header);
        let header = body.block(header.at).expect("header is a block");
        let first = body.block(first.at).expect("first is a block");
        return rotated(&Rc::new(at_body(&body, &loop_, preheader, header, first, &ops, None)?));
    }
    Ok(body.clone())
}

/// Let a zero-ending recurrence's latch step provide the branch flags.
///
/// `rotated` has proved that the preheader will bypass this test, so the
/// header is reached only after the latch update.  When its sole question is
/// whether that updated recurrence is zero, a second compare computes the
/// flags the update already produced.  Keep this in MIR: the relationship is
/// a loop fact, not a post-allocation instruction coincidence.
pub(crate) fn _step_test(body: &Rc<MirBody>, loop_: &Loop, header: &MirBlock) -> Rc<MirBody> {
    if loop_.latches.len() != 1 || header.ops.last().is_none_or(|op| op.kind != Kind::Branch) {
        return body.clone();
    }
    let latch_at = *loop_.latches.iter().next().expect("one latch");
    let latch = body.block(latch_at).expect("latch is a block");
    let branch = header.ops.last().expect("header ends in a branch");
    if !matches!(branch.test, Some(Kind::Eq | Kind::Ne)) {
        return body.clone();
    }
    let made = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (value.id, op)))
        .collect::<BTreeMap<_, _>>();
    let made_at = occurrence::operations(body)
        .flat_map(|(occurrence, _, op)| op.defines.iter().map(move |value| (value.id, occurrence)))
        .collect::<BTreeMap<_, _>>();
    let mut readers = BTreeMap::<Value, Vec<&Op>>::new();
    for block in &body.blocks {
        for op in &block.ops {
            for value in &op.defines {
                readers.insert(*value, Vec::new());
            }
        }
    }
    for block in &body.blocks {
        for op in &block.ops {
            for value in &op.uses {
                readers.entry(*value).or_default().push(op);
            }
        }
    }
    let facts = consts::known(body, None, None, None, None);
    // `header` is one of `body`'s blocks: `op is compare` and `op is branch`
    // are positions in it.
    let header_index = body
        .blocks
        .iter()
        .position(|block| std::ptr::eq(block, header))
        .expect("header belongs to body");

    for counter in induction::basics(body, loop_).values() {
        let Some(phi) = header
            .phis
            .iter()
            .find(|one| one.result.id == counter.value)
        else {
            continue;
        };
        if !phi.incoming.contains_key(&latch_at) {
            continue;
        }
        let width = counter.start.width();
        let comparisons = header.ops[..header.ops.len() - 1]
            .iter()
            .enumerate()
            .filter(|(_, op)| {
                induction::_counter_bound(op, branch, counter, width, Some(&made))
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
        if flags.len() != 1 || readers.get(&flags[0]).is_none_or(|ops| *ops != [branch]) {
            continue;
        }
        let update = phi.incoming.get(&latch_at).copied().expect("KeyError");
        let stepping = made.get(&update.id).copied();
        let stepped = stepping.and_then(mir::stepping);
        let (Some(stepping), Some(stepped)) = (stepping, stepped) else {
            continue;
        };
        if stepped.0
            != Arg::Held(Held {
                value: phi.result,
                width,
            })
            || stepping.results
                != [Arg::Held(Held {
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
        // Python's `latch.ops.index(stepping)`: first structurally equal op,
        // and a ValueError when there is none.
        let step_index = latch
            .ops
            .iter()
            .position(|op| op == stepping)
            .expect("ValueError: stepping is not in list");
        if latch.ops[step_index + 1..]
            .iter()
            .any(|op| !matches!(op.kind, Kind::Nothing | Kind::Jump))
        {
            continue;
        }
        if stepping
            .defines
            .iter()
            .any(|value| value.flags && readers.get(value).is_some_and(|ops| !ops.is_empty()))
        {
            continue;
        }
        // The modular recurrence must reach zero exactly at the proven exit;
        // nonempty() established a finite positive trip count before rotation.
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
        let stepping_at = made_at[&update.id];
        let mut rewritten = Vec::new();
        for (block_index, block) in body.blocks.iter().enumerate() {
            let mut ops = Vec::new();
            for (op_index, op) in block.ops.iter().enumerate() {
                let op = if stepping_at.block_index() == block_index
                    && stepping_at.operation_index() == op_index
                {
                    Op {
                        defines: op.defines.iter().copied().chain([step_flags]).collect(),
                        source_backed: false,
                        raised: None,
                        ..op.clone()
                    }
                } else if block_index == header_index && op_index == compare_index {
                    mir::cleared(op)
                } else if block_index == header_index && op_index == header.ops.len() - 1 {
                    Op {
                        uses: vec![step_flags],
                        source_backed: false,
                        raised: None,
                        ..op.clone()
                    }
                } else {
                    op.clone()
                };
                ops.push(op);
            }
            rewritten.push(MirBlock {
                ops,
                ..block.clone()
            });
        }
        return Rc::new(MirBody {
            blocks: rewritten,
            ..MirBody::clone(body)
        });
    }
    body.clone()
}

/// `body` entering `loop` at `first`, its phis moved there by hand.
///
/// Not re-derived by variable: two values of one variable a pass has
/// already split are not one another's reaching definitions, and renaming
/// `main`'s exit copy of a call's answer to the loop counter let decide
/// fold the exit away.
///
/// A header phi `p(preheader: a, latch: b)` becomes `q(preheader: a,
/// header: b)` at `first`. The loop reads `q`; the test, now reached only
/// from the latch, and everything after the loop read `b`.
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
        .expect("StopIteration: no latch");
    // `ops` may contain values the caller has just constructed for the new
    // preheader and which are not in `body` yet.  Allocate moved phis after
    // both sets.  Looking only at the old body reused a guard flag's id for an
    // accumulator phi; dead-code elimination then erased the accumulator's
    // zero seed and sum read an uninitialized register.
    let serial = ssa::values(body)
        .chain(
            ops.iter()
                .flat_map(|op| op.defines.iter().chain(&op.uses).chain(&op.exits).copied()),
        )
        .map(|value| value.id)
        .max()
        .expect("ValueError: max() arg is an empty sequence")
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

    // A latch value that is itself a header phi is that phi one pass on.
    let latest = |value: Value| moved.get(&value.id).copied().unwrap_or(value);

    let ending = header
        .phis
        .iter()
        .map(|phi| {
            (
                phi.result.id,
                latest(phi.incoming.get(&latch).copied().expect("KeyError")),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let initial = header
        .phis
        .iter()
        .map(|phi| {
            (
                phi.result.id,
                phi.incoming.get(&preheader).copied().expect("KeyError"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let inside = loop_
        .body
        .iter()
        .copied()
        .filter(|at| *at != header.at)
        .collect::<BTreeSet<_>>();
    let entry = header
        .phis
        .iter()
        .map(|phi| Phi {
            result: moved[&phi.result.id],
            incoming: OrderedMap::from_iter([
                (
                    preheader,
                    phi.incoming.get(&preheader).copied().expect("KeyError"),
                ),
                (header.at, ending[&phi.result.id]),
            ]),
        })
        .collect::<Vec<_>>();

    let rewired = |block: &MirBlock| -> Result<MirBlock, SubstitutionError> {
        let swap = if inside.contains(&block.at) {
            &moved
        } else {
            &ending
        };
        let mut phis = Vec::new();
        for phi in &block.phis {
            let mut incoming = phi
                .incoming
                .iter()
                .map(|(at, value)| {
                    let map = if inside.contains(at) { &moved } else { &ending };
                    (*at, map.get(&value.id).copied().unwrap_or(*value))
                })
                .collect::<OrderedMap<_, _>>();
            // A guarded countdown adds a direct zero-trip edge from the
            // preheader to the old exit.  Values which used to arrive there
            // from the header must then be their pre-loop versions, not the
            // latch versions used by the nonzero path.
            if entry_succ.is_some_and(|succ| succ.contains(&block.at)) && block.at != first.at {
                if let Some(zero) = phi.incoming.get(&header.at).filter(|_| !phi.incoming.contains_key(&preheader)) {
                    incoming.insert(preheader, initial.get(&zero.id).copied().unwrap_or(*zero));
                }
            }
            phis.push(Phi {
                incoming,
                ..phi.clone()
            });
        }
        let changed = MirBlock {
            phis,
            ops: block
                .ops
                .iter()
                .map(|op| _swapped(op, swap))
                .collect::<Result<_, _>>()?,
            ..block.clone()
        };
        if block.at == preheader {
            let succ = match entry_succ {
                Some(succ) if !succ.is_empty() => succ.to_vec(),
                _ => vec![first.at],
            };
            return Ok(MirBlock {
                ops: ops.to_vec(),
                succ,
                ..changed
            });
        }
        if block.at == header.at {
            return Ok(MirBlock {
                phis: Vec::new(),
                ..changed
            });
        }
        if block.at == first.at {
            return Ok(MirBlock {
                phis: entry.clone(),
                ..changed
            });
        }
        Ok(changed)
    };

    let mut counts = body
        .loop_trip_counts
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();
    if let Some(count) = counts.remove(&header.at) {
        // Rotation makes `first` the natural-loop header.  Preserve an
        // exact fact only when it does not collide with a distinct loop fact;
        // losing a measurement is preferable to attaching the wrong count.
        if counts.get(&first.at).is_none_or(|other| *other == count) {
            counts.insert(first.at, count);
        }
    }
    let mut integer_ranges = body.integer_ranges.clone();
    for phi in &header.phis {
        if let Some(interval) = integer_ranges.remove(&phi.result) {
            integer_ranges.insert(moved[&phi.result.id], interval);
        }
    }
    Ok(MirBody {
        blocks: body.blocks.iter().map(rewired).collect::<Result<_, _>>()?,
        integer_ranges,
        loop_trip_counts: counts.into_iter().collect(),
        ..body.clone()
    })
}

/// One substitution step, never chained: a replacement is not itself replaced.
pub(crate) fn _swapped(op: &Op, swap: &BTreeMap<u32, Value>) -> Result<Op, SubstitutionError> {
    if swap.is_empty() || !op.uses.iter().any(|value| swap.contains_key(&value.id)) {
        return Ok(op.clone());
    }
    ssa::substituted(
        op,
        &swap
            .iter()
            .filter(|(key, value)| !swap.contains_key(&value.id) || value.id == **key)
            .map(|(key, value)| (*key, *value))
            .collect(),
    )
}

#[cfg(test)]
#[path = "rotate_tests.rs"]
mod tests;
