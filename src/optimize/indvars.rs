//! Use an existing recurrence to control a loop instead of a redundant counter.
//!
//! Direct port of `qbopt/optimize/indvars.py`.  Python's `id(op)` and `op is
//! other` become the operation's `(block index, operation index)` in the body
//! it was read from.  After `loopexit._substituted_exits` that identity holds
//! only outside `following`: Python rebuilds every operation there and keeps
//! the rest.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive};

use crate::analysis::consts::{self, Known};
use crate::analysis::induction::{self, Affine, AffineOperand};
use crate::analysis::ssa::{self, SubstitutionError};
use crate::analysis::{liveness, loops};
use crate::model::mir::{self, Arg, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OrderedMap, Phi, Value};
use crate::model::passes::OperationCosts;

use super::{counting, loopexit, rotate, strength, transform};

/// Reuse an exact inner recurrence instead of reloading its saved start.
///
/// An inner recurrence which runs exactly `count` times leaves through its
/// sole exit as `start + count * step`.  If that loop is itself repeated,
/// subtracting the proven distance on the outer backedge reconstructs the
/// next invocation's start and makes the separately saved start dead across
/// the hot inner loop.
///
/// This is deliberately a pressure-and-target decision.  In a register the
/// old copy and the rewind are equivalent work; when pressure puts both ends
/// in frame cells, the old form is a load plus a store and the new form is a
/// memory update.  The 386/486/P5 profiles price the latter higher and retain
/// the copy.  Later profiles may take it.  Nothing here names either form.
pub(crate) fn rewound(body: &Rc<MirBody>, registers: i64, costs: Option<&OperationCosts>) -> Rc<MirBody> {
    let default_costs;
    let costs = match costs {
        Some(costs) => costs,
        None => {
            default_costs = OperationCosts::default();
            &default_costs
        }
    };
    if registers == 0 || costs.add > costs.r#move || costs.memory_update > costs.load + costs.store {
        return body.clone();
    }

    let found = loops::loops(&body.blocks, Some(body.entry));
    if found.len() < 2 {
        return body.clone();
    }
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(&body.blocks);
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let facts = consts::known(body, None, None, None, None);
    let live = liveness::live(body);
    let values = ssa::values(body).collect::<Vec<_>>();
    let definitions = body
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .ops
                .iter()
                .flat_map(move |op| op.defines.iter().map(move |value| (*value, (block.at, op))))
        })
        .collect::<BTreeMap<_, _>>();
    let nowhere = BTreeSet::new();

    for inner in &found {
        let parents = found
            .iter()
            .filter(|parent| {
                inner.body.is_subset(&parent.body) && inner.body != parent.body && parent.body.contains(&inner.header)
            })
            .collect::<Vec<_>>();
        if parents.is_empty() || inner.latches.len() != 1 {
            continue;
        }
        let parent = *parents.iter().min_by_key(|parent| parent.body.len()).expect("parents is not empty");
        if parent.latches.len() != 1 {
            continue;
        }
        let inner_preheader = transform::_preheader(body, inner);
        let parent_preheader = transform::_preheader(body, parent);
        let inner_latch = *inner.latches.iter().next().expect("one latch");
        let parent_latch = *parent.latches.iter().next().expect("one latch");
        let (Some(inner_preheader), Some(parent_preheader)) = (inner_preheader, parent_preheader) else {
            continue;
        };
        if !parent.body.contains(&inner_preheader)
            || blocks[&inner_preheader].succ != [inner.header]
            || blocks[&parent_preheader].succ != [parent.header]
            || predecessors.get(&parent.header) != Some(&BTreeSet::from([parent_preheader, parent_latch]))
            || (liveness::pressure(body, Some(&live), Some(&inner.body)) as i64) < registers
        {
            continue;
        }

        let exiting = body
            .blocks
            .iter()
            .filter(|block| inner.body.contains(&block.at))
            .flat_map(|block| {
                block
                    .succ
                    .iter()
                    .filter(|successor| blocks.contains_key(successor) && !inner.body.contains(successor))
                    .map(move |successor| (block.at, *successor))
            })
            .collect::<Vec<_>>();
        let exits = exiting.iter().map(|(_source, target)| *target).collect::<BTreeSet<_>>();
        if exits.len() != 1 {
            continue;
        }
        let exit_at = *exits.iter().next().expect("one exit");
        let sources = exiting.iter().map(|(source, _target)| *source).collect::<BTreeSet<_>>();
        if !parent.body.contains(&exit_at)
            || predecessors.get(&exit_at) != Some(&sources)
            || !dominators.get(&parent_latch).unwrap_or(&nowhere).contains(&exit_at)
        {
            continue;
        }

        let Some(count) = induction::trip_count(body, inner, &facts) else {
            continue;
        };
        let header = blocks[&inner.header];
        let basics = induction::basics(body, inner);
        for counter in basics.values() {
            let Some(phi) = header.phis.iter().find(|one| one.result.id == counter.value) else {
                continue;
            };
            if phi.incoming.keys().copied().collect::<BTreeSet<_>>() != BTreeSet::from([inner_preheader, inner_latch]) {
                continue;
            }
            let start = *phi.incoming.get(&inner_preheader).expect("checked incoming");
            let update = *phi.incoming.get(&inner_latch).expect("checked incoming");
            let width = counter.start.width();
            let step = induction::_signed(&counter.step.as_arg(), &facts, width);
            let update_at = definitions.get(&update);
            let start_definition = definitions.get(&start);
            let (Some(step), Some(update_at), Some(start_definition)) = (step, update_at, start_definition) else {
                continue;
            };
            if step == BigInt::from(0_u8) || parent.body.contains(&start_definition.0) || facts.contains_key(&start) {
                continue;
            }
            // A pre-tested loop exits from its header before executing the
            // next update.  Its phi is already `start + count * step` and
            // dominates that edge.  A post-tested loop exits from the latch,
            // where the update itself is the corresponding value.  Do not
            // demand the latter dominate an intentionally zero-trip-capable
            // header merely because both shapes share one recurrence proof.
            let exit_value = if sources
                .iter()
                .all(|source| dominators.get(source).unwrap_or(&nowhere).contains(&update_at.0))
            {
                update
            } else if sources == BTreeSet::from([inner.header]) {
                phi.result
            } else {
                continue;
            };
            // The saved start must become dead in the enclosing loop.  Other
            // uses would keep its live range and turn an equal-cost register
            // rewrite into a pure code-size loss.
            let enclosed = || body.blocks.iter().filter(|block| parent.body.contains(&block.at));
            if enclosed().flat_map(|block| &block.ops).any(|op| op.uses.contains(&start))
                || enclosed()
                    .flat_map(|block| &block.phis)
                    .any(|other| other.incoming.values().any(|value| *value == start) && !std::ptr::eq(other, phi))
            {
                continue;
            }

            let mask = (BigInt::from(1_u8) << (width * 8)) - 1;
            let distance = (&step * &count) & &mask;
            if distance == BigInt::from(0_u8) {
                continue;
            }
            let next_id = values.iter().map(|value| i64::from(value.id)).max().unwrap_or(-1) + 1;
            let next_variable = values.iter().map(|value| i64::from(value.variable)).max().unwrap_or(-1) + 1;
            let next_version = values
                .iter()
                .filter(|value| value.variable == exit_value.variable)
                .map(|value| value.version)
                .max()
                .unwrap_or(0)
                + 1;
            let next_id = u32::try_from(next_id).expect("value ids are nonnegative");
            let next_variable = u32::try_from(next_variable).expect("variables are nonnegative");
            let seed = Value { id: next_id, at: parent_preheader, flags: false, variable: next_variable, version: 1 };
            let closed = Value {
                id: next_id + 1,
                at: exit_at,
                flags: false,
                variable: exit_value.variable,
                version: next_version,
            };
            let reset = Value { id: next_id + 2, at: exit_at, flags: false, variable: next_variable, version: 2 };
            let carried =
                Value { id: next_id + 3, at: parent.header, flags: false, variable: next_variable, version: 3 };
            let seeded = strength::_made(
                Kind::Copy,
                "",
                seed,
                vec![Arg::Held(Held { value: start, width })],
                blocks[&parent_preheader].ops.last().map_or(parent_preheader, |op| op.at),
                start_definition.1,
            );
            let rewind = strength::_made(
                Kind::Add,
                "add",
                reset,
                vec![Arg::Held(Held { value: closed, width }), Arg::Const(Const::new((-&distance) & &mask, width))],
                exit_at,
                update_at.1,
            );

            let mut changed = Vec::new();
            for block in &body.blocks {
                let mut phis = block.phis.clone();
                let mut ops = block.ops.clone();
                if block.at == parent_preheader {
                    _before_leaving(&mut ops, vec![seeded.clone()]);
                }
                if block.at == parent.header {
                    phis.push(Phi {
                        result: carried,
                        incoming: OrderedMap::from_iter([(parent_preheader, seed), (parent_latch, reset)]),
                    });
                }
                if block.at == inner.header {
                    for (index, other) in phis.iter_mut().enumerate() {
                        if block.phis.get(index).is_some_and(|original| std::ptr::eq(original, phi)) {
                            other.incoming.insert(inner_preheader, carried);
                        }
                    }
                }
                if block.at == exit_at {
                    phis.push(Phi {
                        result: closed,
                        incoming: sources.iter().map(|source| (*source, exit_value)).collect(),
                    });
                    _before_leaving(&mut ops, vec![rewind.clone()]);
                }
                changed.push(MirBlock { at: block.at, phis, ops, succ: block.succ.clone(), cold: block.cold });
            }
            let mut counts = body.loop_trip_counts.iter().copied().collect::<BTreeMap<_, _>>();
            counts.insert(inner.header, i64::try_from(&count).expect("trip count fits the MIR table"));
            return Rc::new(MirBody { loop_trip_counts: counts.into_iter().collect(), ..body.with_blocks(changed) });
        }
    }
    body.clone()
}

pub(crate) fn simplified(body: &Rc<MirBody>) -> Result<Rc<MirBody>, SubstitutionError> {
    let facts = consts::known(body, None, None, None, None);
    let blocks = body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect::<BTreeMap<_, _>>();
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let predecessors = loops::predecessors(&body.blocks);
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        if loop_.latches.len() != 1 {
            continue;
        }
        let preheader = transform::_preheader(body, &loop_);
        let Some(preheader) = preheader.filter(|preheader| body.blocks[blocks[preheader]].succ == [loop_.header])
        else {
            continue;
        };
        let latch = *loop_.latches.iter().next().expect("one latch");
        let header_index = blocks[&loop_.header];
        let header = &body.blocks[header_index];
        let counters = induction::basics(body, &loop_);
        for proof in induction::counted(body, &loop_, Some(&facts), false) {
            if proof.posttested || proof.first.is_none() || proof.width() != proof.counter.start.width() {
                continue;
            }
            let (counter, phi) = (&proof.counter, proof.phi_in(body));
            let count = proof.count.clone().expect("a span has a count");
            let (last, step) = (proof.last.clone().expect("a span has a last"), proof.step.clone());
            let width = counter.start.width();
            let update = *phi.incoming.get(&proof.latch).expect("a latch input");
            let (branch_index, compare_index) = (proof.branch.operation_index(), proof.compare.operation_index());
            let (branch, compare) = (proof.branch_in(body), proof.compare_in(body));
            let [exit_at] = header.succ.iter().copied().filter(|at| !loop_.body.contains(at)).collect::<Vec<_>>()[..]
            else {
                panic!("one exit from the header");
            };
            let exit_block = &body.blocks[blocks[&exit_at]];
            if predecessors.get(&exit_at).cloned().unwrap_or_default() != BTreeSet::from([header.at])
                || exit_block.ops.is_empty()
            {
                continue;
            }
            let mut closed = IndexMap::<Value, &Phi>::default();
            for other in &exit_block.phis {
                if other.incoming.len() == 1 {
                    for (predecessor, value) in other.incoming.iter() {
                        if loop_.body.contains(predecessor) {
                            closed.insert(*value, other);
                        }
                    }
                }
            }
            let following = blocks
                .keys()
                .copied()
                .filter(|at| dominators.get(at).is_some_and(|dominating| dominating.contains(&exit_at)))
                .collect::<BTreeSet<_>>();
            let leaving = transform::_leaving(body);
            if [phi.result, update].iter().any(|value| leaving.contains(value)) {
                continue;
            }
            if body.blocks.iter().flat_map(|block| &block.ops).any(|op| op.uses.contains(&update)) {
                continue;
            }
            if body.blocks.iter().flat_map(|block| &block.phis).any(|other| {
                (other.incoming.values().any(|value| *value == phi.result)
                    || other.incoming.values().any(|value| *value == update))
                    && !std::ptr::eq(other, phi)
                    && !closed.values().any(|one| *one == other)
            }) {
                continue;
            }
            if body.blocks.iter().flat_map(|block| &block.ops).any(|op| {
                compare.defines.iter().any(|value| op.uses.contains(value)) && !std::ptr::eq(op, branch)
            }) {
                continue;
            }
            if body.blocks.iter().flat_map(|block| &block.phis).any(|other| {
                compare.defines.iter().any(|value| other.incoming.values().any(|incoming| incoming == value))
            }) {
                continue;
            }
            for alternative in counters.values() {
                let alternative_width = alternative.start.width();
                let stride = induction::_signed(&alternative.step.as_arg(), &facts, alternative_width);
                let modulus = BigInt::from(1_u8) << (8 * alternative_width);
                if alternative.value == counter.value {
                    continue;
                }
                let Some(stride) = stride.filter(|stride| *stride != BigInt::from(0_u8)) else {
                    continue;
                };
                if count >= &modulus / induction::gcd(stride.abs(), modulus.clone()) {
                    continue;
                }
                let alternative_phi = header
                    .phis
                    .iter()
                    .find(|phi| phi.result.id == alternative.value)
                    .expect("a basic counter's phi");
                let value = alternative_phi.result;
                let alternative_update = *alternative_phi.incoming.get(&latch).expect("a latch input");
                if !body
                    .blocks
                    .iter()
                    .filter(|block| loop_.body.contains(&block.at))
                    .flat_map(|block| &block.ops)
                    .any(|op| op.uses.contains(&value) && !op.defines.contains(&alternative_update))
                {
                    continue;
                }
                let rebased =
                    _rebased_equalities(body, phi.result, update, (header_index, compare_index), value, &following);
                let Some(rebased) = rebased else {
                    continue;
                };
                let serial = ssa::values(body).map(|value| value.id).max().expect("a value") + 1;
                let variable = ssa::values(body).map(|value| value.variable).max().expect("a value") + 1;
                let preheader_last = body.blocks[blocks[&preheader]].ops.last().expect("a preheader operation");
                let seed_at = preheader_last.at;
                let bound = Value { id: serial, at: seed_at, flags: false, variable, version: 0 };
                let final_ = Value { id: serial + 1, at: exit_at, flags: false, variable: variable + 1, version: 0 };
                let seed = strength::_made(
                    Kind::Add,
                    "",
                    bound,
                    vec![
                        alternative.start.as_arg(),
                        Arg::Const(Const::new(consts::masked(&(&stride * &count), alternative_width), alternative_width)),
                    ],
                    seed_at,
                    preheader_last,
                );
                let finish = strength::_made(
                    Kind::Copy,
                    "",
                    final_,
                    vec![Arg::Const(Const::new(consts::masked(&(&last + &step), width), width))],
                    exit_at,
                    &exit_block.ops[0],
                );
                let mut swap = BTreeMap::from([(counter.value, final_)]);
                let removed = closed
                    .iter()
                    .filter(|(value, _other)| **value == phi.result || **value == update)
                    .map(|(_value, other)| other.result)
                    .collect::<BTreeSet<_>>();
                swap.extend(removed.iter().map(|value| (value.id, final_)));
                let mut changed = loopexit::_substituted_exits(body, exit_at, &following, &[finish], &swap)?;
                if !removed.is_empty() {
                    for block in &mut changed.blocks {
                        block.phis.retain(|other| !removed.contains(&other.result));
                    }
                }
                let mut out = Vec::new();
                for (block_index, block) in changed.blocks.iter().enumerate() {
                    let kept = !following.contains(&block.at);
                    let mut ops = Vec::new();
                    for (operation_index, op) in block.ops.iter().enumerate() {
                        let at = (block_index, operation_index);
                        let op = if kept && at == (header_index, compare_index) {
                            let mut op = op.clone();
                            op.args = vec![
                                Arg::Held(Held { value, width: alternative_width }),
                                Arg::Held(Held { value: bound, width: alternative_width }),
                            ];
                            op.kind = Kind::Sub;
                            op.results = Vec::new();
                            op.defines.retain(|value| value.flags);
                            op.uses = vec![value, bound];
                            op.loads = Vec::new();
                            op.source_backed = false;
                            op.raised = None;
                            op
                        } else if kept && at == (header_index, branch_index) {
                            let mut op = op.clone();
                            op.test = Some(if branch.target.is_some_and(|target| loop_.body.contains(&target)) {
                                Kind::Ne
                            } else {
                                Kind::Eq
                            });
                            op.name.clear();
                            op.source_backed = false;
                            op.raised = Some((Vec::new(), Vec::new()));
                            op
                        } else if kept {
                            rebased.get(&at).cloned().unwrap_or_else(|| op.clone())
                        } else {
                            op.clone()
                        };
                        ops.push(op);
                    }
                    if block.at == preheader {
                        _before_leaving(&mut ops, vec![seed.clone()]);
                    }
                    out.push(block.with_ops(ops));
                }
                return Ok(Rc::new(MirBody { blocks: out, ..changed }));
            }
        }
    }
    Ok(body.clone())
}

/// Every basic recurrence, with its width and proven domain when finite.
fn _recurrences(
    body: &Rc<MirBody>,
    facts: &IndexMap<Value, Known>,
) -> IndexMap<u32, (Affine, u32, Option<BigInt>, Option<BigInt>)> {
    let mut out = IndexMap::default();
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        for affine in induction::basics(body, &loop_).values() {
            let width = affine.start.width();
            let (low, high) = match induction::domain(body, &loop_, affine, facts) {
                Some((low, high)) => (Some(low), Some(high)),
                None => (None, None),
            };
            out.insert(affine.value, (affine.clone(), width, low, high));
        }
    }
    out
}

/// Rewrite equality-only body uses through an injective recurrence.
///
/// Strength reduction commonly leaves both `i` and `i * element_size`
/// live.  The scaled recurrence may control the loop, but only if every other
/// use of `i` can use it too.  Equality with another finite recurrence is
/// such a use when both sides have the same affine map and their combined
/// proven domain is shorter than the map's modular period.
fn _rebased_equalities(
    body: &Rc<MirBody>,
    counter: Value,
    update: Value,
    control: (usize, usize),
    alternative: Value,
    following: &BTreeSet<i64>,
) -> Option<BTreeMap<(usize, usize), Op>> {
    let extra = body
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| !following.contains(&block.at))
        .flat_map(|(block_index, block)| {
            block.ops.iter().enumerate().map(move |(operation_index, op)| ((block_index, operation_index), op))
        })
        .any(|(at, op)| op.uses.contains(&counter) && at != control && !op.defines.contains(&update));
    if !extra {
        // The original IndVarSimplify case: any sufficiently long-lived
        // recurrence can terminate the loop when the old counter has no other
        // purpose.  No affine relationship between them is required.
        return Some(BTreeMap::new());
    }
    let facts = consts::known(body, None, None, None, None);
    let recurrences = _recurrences(body, &facts);
    let source = recurrences.get(&counter.id)?;
    let target = recurrences.get(&alternative.id)?;
    let (Some(source_low), Some(source_high)) = (&source.2, &source.3) else {
        return None;
    };
    let relation = induction::relation(&source.0, &target.0, &facts)?;
    let width = source.1;
    let flags = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().copied())
        .filter(|value| value.flags)
        .collect::<BTreeSet<_>>();
    let flags_readers = ssa::use_index(body, Some(&flags), false);
    let mut replacements = BTreeMap::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        if following.contains(&block.at) {
            continue;
        }
        for (operation_index, op) in block.ops.iter().enumerate() {
            let at = (block_index, operation_index);
            if !op.uses.contains(&counter) || at == control || op.defines.contains(&update) {
                continue;
            }
            if op.kind != Kind::Sub
                || !op.results.is_empty()
                || !op.loads.is_empty()
                || !op.stores.is_empty()
                || op.barrier()
                || !op.merges.is_empty()
                || op.args.len() != 2
                || op.defines.len() != 1
                || !op.defines[0].flags
                || flags_readers.get(&op.defines[0]).is_none_or(|readers| readers.is_empty())
                || flags_readers[&op.defines[0]].iter().any(|reader| {
                    let reader = &body.blocks[reader.block_index()].ops[reader.operation_index()];
                    reader.kind != Kind::Branch || !matches!(reader.test, Some(Kind::Eq | Kind::Ne))
                })
            {
                return None;
            }
            let positions = op
                .args
                .iter()
                .enumerate()
                .filter(|(_, arg)| matches!(arg, Arg::Held(held) if held.value == counter && held.width == width))
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if positions.len() != 1 {
                return None;
            }
            let other_at = 1 - positions[0];
            let Arg::Held(other) = op.args[other_at] else {
                return None;
            };
            if other.width != width {
                return None;
            }
            let other_source = recurrences.get(&other.value.id)?;
            let (Some(other_low), Some(other_high)) = (&other_source.2, &other_source.3) else {
                return None;
            };
            let partner_id = recurrences
                .iter()
                .find(|(value, candidate)| {
                    **value != other.value.id
                        && candidate.0.header == other_source.0.header
                        && induction::relation(&other_source.0, &candidate.0, &facts).as_ref() == Some(&relation)
                })
                .map(|(value, _)| *value)?;
            let low = source_low.min(other_low);
            let high = source_high.max(other_high);
            if !relation.injective(low, high) {
                return None;
            }
            let actual = body
                .blocks
                .iter()
                .flat_map(|block| &block.phis)
                .map(|phi| phi.result)
                .find(|value| value.id == partner_id)
                .expect("the partner recurrence's phi");
            let mut args = op.args.clone();
            args[positions[0]] = Arg::Held(Held { value: alternative, width });
            args[other_at] = Arg::Held(Held { value: actual, width });
            let uses = op
                .uses
                .iter()
                .map(|value| {
                    if *value == counter {
                        alternative
                    } else if *value == other.value {
                        actual
                    } else {
                        *value
                    }
                })
                .collect();
            let mut replacement = op.clone();
            replacement.args = args;
            replacement.uses = uses;
            replacement.source_backed = false;
            replacement.raised = None;
            replacements.insert(at, replacement);
        }
    }
    Some(replacements)
}

/// `inserted` at the end of a preheader, before the jump it leaves by if it has one.
///
/// A preheader that falls through ends on an operation like any other, and
/// the seed may read what that one defines.
fn _before_leaving(ops: &mut Vec<Op>, inserted: Vec<Op>) {
    let cut = ops.len() - usize::from(ops.last().is_some_and(|op| matches!(op.kind, Kind::Jump | Kind::Branch)));
    ops.splice(cut..cut, inserted);
}

/// Use a bounded affine data recurrence as the loop's sole control.
///
/// For a counted loop of `n` trips and an existing recurrence from `r0`
/// with stride `s`, rebase its invariant users by its final value
/// `r0 + n*s` and start it at `-n*s`.  Its update reaches zero on
/// exactly the final iteration, so the original unit counter disappears.
/// The proof is target-independent: `induction.counted` supplies `n` and
/// `AffineMap.period` the modular safety condition.
pub(crate) fn symbolically_zeroed(body: &Rc<MirBody>) -> Result<Rc<MirBody>, SubstitutionError> {
    let facts = consts::known(body, None, None, None, None);
    let blocks = body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect::<BTreeMap<_, _>>();
    let operation = |at: (usize, usize)| &body.blocks[at.0].ops[at.1];
    let mut made = BTreeMap::new();
    let mut readers = BTreeMap::<Value, Vec<(usize, usize)>>::new();
    let mut placed = BTreeMap::new();
    let mut home = BTreeMap::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        for (operation_index, op) in block.ops.iter().enumerate() {
            let at = (block_index, operation_index);
            for value in &op.defines {
                made.insert(*value, at);
                home.insert(*value, block.at);
            }
            for value in &op.uses {
                readers.entry(*value).or_default().push(at);
            }
            placed.insert(at, block.at);
        }
    }
    home.extend(body.blocks.iter().flat_map(|block| block.phis.iter().map(move |phi| (phi.result, block.at))));
    let values = ssa::values(body).collect::<Vec<_>>();

    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let proofs = induction::counted(body, &loop_, Some(&facts), true);
        if proofs.len() != 1 {
            continue;
        }
        let proof = &proofs[0];
        let header = &body.blocks[blocks[&loop_.header]];
        let inside = loop_.body.clone();
        let candidates = induction::basics(body, &loop_);
        for candidate in candidates.values() {
            let Some(preheader) = proof.preheader else {
                continue;
            };
            let Some(phi) = header.phis.iter().find(|one| one.result.id == candidate.value) else {
                continue;
            };
            if phi.incoming.keys().copied().collect::<BTreeSet<_>>() != BTreeSet::from([preheader, proof.latch]) {
                continue;
            }
            let initial = *phi.incoming.get(&preheader).expect("checked incoming");
            let update = *phi.incoming.get(&proof.latch).expect("checked incoming");
            let Some(stepping_at) = made.get(&update).copied() else {
                continue;
            };
            let stepping = operation(stepping_at);
            if stepping.results.is_empty()
                || !matches!(stepping.results[0], Arg::Held(_))
                || !stepping.loads.is_empty()
                || !stepping.stores.is_empty()
                || stepping.barrier()
                || !stepping.merges.is_empty()
            {
                continue;
            }
            // The counter itself may take the zero test: its own compare is
            // control, and every other read must then be an offset.
            let own_compare = (proof.compare.block_index(), proof.compare.operation_index());
            let itself = candidate == &proof.counter;
            let own = if itself { BTreeSet::from([stepping_at, own_compare]) } else { BTreeSet::from([stepping_at]) };
            let offsets = _offsets(phi.result, &readers, &placed, &home, &inside, &own, body, true);
            // A direct address recurrence has no separate invariant base to
            // rebase.  It remains valid, but cannot replace control by this
            // representation.  This is a property of the affine expression,
            // not of an instruction or register class.
            let Some(offsets) = offsets else {
                continue;
            };
            if offsets.is_empty() || offsets.iter().any(|(_op, position, _multiplier, _address, _extra)| position.is_none())
            {
                continue;
            }
            let covered = if itself {
                let rebased = offsets.iter().map(|(at, ..)| *at).collect::<BTreeSet<_>>();
                crate::analysis::occurrence::operations(body)
                    .map(|(at, ..)| at)
                    .filter(|at| rebased.contains(&(at.block_index(), at.operation_index())))
                    .collect()
            } else {
                BTreeSet::new()
            };
            let Some(symbolic) =
                induction::zero_terminating_control(body, &loop_, proof, candidate, &covered, Some(&facts))
            else {
                continue;
            };
            let control = &symbolic.replacement;
            let width = symbolic.candidate.start.width();
            let step = &symbolic.step;
            let read = body
                .blocks
                .iter()
                .flat_map(|block| &block.ops)
                .flat_map(|op| op.uses.iter().copied())
                .collect::<BTreeSet<_>>();
            if read.contains(&initial) || read.contains(&update) {
                continue;
            }
            if stepping.defines.iter().any(|value| value.flags && read.contains(value)) {
                continue;
            }

            // Each offset is rebased at the width its operation reads: the
            // counter's low bytes, never more than the counter has.
            let reading = |at: (usize, usize), position: usize| match &operation(at).args[position] {
                Arg::Held(held) => held.width,
                Arg::Const(constant) => constant.width,
                _ => width,
            };
            if offsets.iter().any(|(at, position, ..)| reading(*at, position.expect("checked above")) > width) {
                continue;
            }
            let compare = &body.blocks[proof.compare.block_index()].ops[proof.compare.operation_index()];
            let ending = body.blocks[blocks[&preheader]].ops.last().unwrap_or(compare);
            let mut builder = counting::Seeds {
                serial: values.iter().map(|value| value.id).max().unwrap_or(0) + 1,
                variable: values.iter().map(|value| value.variable).max().unwrap_or(0) + 1,
                at: ending.at,
                width,
                ops: Vec::new(),
            };

            let Some(count) = induction::trips(proof, &mut |kind, args| builder.computed(kind, args)) else {
                continue;
            };
            let distance = builder.computed(
                Kind::Mul,
                vec![count.as_arg(), Arg::Const(Const::new(consts::masked(step, width), width))],
            );
            let final_ = builder.computed(Kind::Add, vec![Arg::Held(Held { value: initial, width }), distance.as_arg()]);
            let mut rebased = BTreeMap::<(usize, usize), Op>::new();
            for (at, position, multiplier, _address, _extra) in &offsets {
                let position = position.expect("offsets were checked for a position");
                let op = operation(*at);
                let base = op.args[position].clone();
                let delta = builder.computed(
                    Kind::Mul,
                    vec![final_.as_arg(), Arg::Const(Const::new(consts::masked(multiplier, width), width))],
                );
                let narrow = reading(*at, position);
                let delta = match delta {
                    AffineOperand::Held(held) => Arg::Held(Held { width: narrow, ..held }),
                    AffineOperand::Const(constant) => Arg::Const(Const::new(consts::masked(&constant.n, narrow), narrow)),
                };
                builder.width = narrow;
                let adjusted = builder.computed(Kind::Add, vec![base.clone(), delta]);
                builder.width = width;
                let args = op
                    .args
                    .iter()
                    .enumerate()
                    .map(|(index, arg)| if index == position { adjusted.as_arg() } else { arg.clone() })
                    .collect();
                assert!(matches!(base, Arg::Held(_) | Arg::Const(_)));
                let mut uses: Vec<Value> = op
                    .uses
                    .iter()
                    .map(|value| match &base {
                        Arg::Held(base) if *value == base.value => match &adjusted {
                            AffineOperand::Held(adjusted) => adjusted.value,
                            AffineOperand::Const(_) => panic!("AttributeError: 'Const' object has no attribute 'value'"),
                        },
                        _ => *value,
                    })
                    .collect();
                // A constant offset that became a held one is a new read, placed
                // among the held arguments where it now sits.
                if let (Arg::Const(_), AffineOperand::Held(adjusted)) = (&base, &adjusted) {
                    let before = op.args[..position].iter().filter(|arg| matches!(arg, Arg::Held(_))).count();
                    uses.insert(before.min(uses.len()), adjusted.value);
                }
                let mut replacement = op.clone();
                replacement.args = args;
                replacement.uses = uses;
                replacement.source_backed = false;
                replacement.raised = None;
                rebased.insert(*at, replacement);
            }

            let computed = builder.computed(Kind::Sub, vec![Arg::Const(Const::new(0, width)), distance.as_arg()]);
            let begun = builder.held(computed);
            let exits = counting::leaving(body, control, &mut builder);
            let step_flags =
                Value { id: builder.serial, at: stepping.at, flags: true, variable: builder.variable, version: 1 };
            let guard_flags =
                Value { id: builder.serial + 1, at: ending.at, flags: true, variable: builder.variable + 1, version: 1 };
            let mut decrement = stepping.clone();
            decrement.name.clear();
            decrement.defines =
                stepping.defines.iter().copied().filter(|value| !value.flags).chain([step_flags]).collect();
            decrement.source_backed = false;
            decrement.raised = None;
            decrement.symbol = Some(false);
            let (guard_compare, guard_branch) = counting::skip_guard(body, proof, ending.at, guard_flags);
            let proof_phi = &body.blocks[proof.phi.block_index()].phis[proof.phi.phi_index()];
            let preheader_input = *proof_phi.incoming.get(&preheader).expect("a preheader input");
            let private = [Some(initial), exits.is_empty().then_some(preheader_input)]
                .into_iter()
                .flatten()
                .filter_map(|value| {
                    let definition = made.get(&value).copied()?;
                    (!body.blocks.iter().flat_map(|block| &block.ops).any(|op| op.uses.contains(&value))
                        && !builder.ops.iter().chain([&guard_compare]).any(|op| op.uses.contains(&value))
                        && !body.blocks.iter().flat_map(|block| &block.phis).any(|other| {
                            other.incoming.values().any(|incoming| *incoming == value)
                                && !std::ptr::eq(other, phi)
                                && !std::ptr::eq(other, proof_phi)
                        }))
                    .then_some(definition)
                })
                .collect::<Vec<_>>();
            let preheader_index = blocks[&preheader];
            let mut entry_ops = body.blocks[preheader_index]
                .ops
                .iter()
                .enumerate()
                .map(|(index, op)| {
                    if private.contains(&(preheader_index, index)) { mir::cleared(op) } else { op.clone() }
                })
                .collect::<Vec<_>>();
            match entry_ops.last().map(|op| op.kind) {
                Some(Kind::Jump) => {
                    let last = entry_ops.len() - 1;
                    entry_ops[last] = mir::cleared(&entry_ops[last]);
                }
                Some(Kind::Branch) => continue,
                _ => {}
            }
            _before_leaving(&mut entry_ops, builder.ops);
            entry_ops.extend([guard_compare, guard_branch]);

            let compare_at = (proof.compare.block_index(), proof.compare.operation_index());
            let branch_at = (proof.branch.block_index(), proof.branch.operation_index());
            let control_stepping = (control.stepping.block_index(), control.stepping.operation_index());
            let mut rewritten = Vec::new();
            for (block_index, block) in body.blocks.iter().enumerate() {
                let mut ops = Vec::new();
                for (operation_index, op) in block.ops.iter().enumerate() {
                    let at = (block_index, operation_index);
                    if at == stepping_at || at == control_stepping {
                        continue;
                    }
                    let op = if at == compare_at || private.contains(&at) {
                        mir::cleared(op)
                    } else if at == branch_at {
                        let mut op = op.clone();
                        op.name.clear();
                        op.uses = vec![step_flags];
                        op.source_backed = false;
                        op.test = Some(Kind::Ne);
                        op.target = Some(proof.latch);
                        op.raised = None;
                        op.symbol = Some(false);
                        op
                    } else {
                        rebased.get(&at).cloned().unwrap_or_else(|| op.clone())
                    };
                    ops.push(op);
                }
                if block.at == proof.latch {
                    let cut = ops.len() - usize::from(ops.last().is_some_and(|op| op.kind == Kind::Jump));
                    ops.insert(cut, decrement.clone());
                }
                let exit_phi = |phi_index: usize, other: &Phi| {
                    exits
                        .iter()
                        .find(|(at, _)| at.block_index() == block_index && at.phi_index() == phi_index)
                        .map_or_else(|| other.clone(), |(_, exit)| exit.clone())
                };
                let phis = if block.at == loop_.header {
                    block
                        .phis
                        .iter()
                        .enumerate()
                        .filter(|(_, other)| !std::ptr::eq(*other, proof_phi) || std::ptr::eq(*other, phi))
                        .map(|(phi_index, other)| {
                            if std::ptr::eq(other, phi) {
                                Phi {
                                    result: other.result,
                                    incoming: OrderedMap::from_iter([
                                        (preheader, begun.value),
                                        (proof.latch, update),
                                    ]),
                                }
                            } else {
                                exit_phi(phi_index, other)
                            }
                        })
                        .collect()
                } else {
                    block.phis.iter().enumerate().map(|(phi_index, other)| exit_phi(phi_index, other)).collect()
                };
                rewritten.push(MirBlock { at: block.at, phis, ops, succ: block.succ.clone(), cold: block.cold });
            }
            let changed = body.with_blocks(rewritten);
            let rotated = rotate::at_body(
                &changed,
                &loop_,
                preheader,
                changed.block(loop_.header).expect("the header remains"),
                changed.block(proof.latch).expect("the latch remains"),
                &entry_ops,
                Some(&[proof.latch, proof.exit]),
                induction::trip_count(body, &loop_, &facts).and_then(|count| count.to_i64()),
            )?;
            return symbolically_zeroed(&Rc::new(rotated));
        }
    }
    Ok(body.clone())
}

/// A counter counted up to zero, so the loop can end on its step's flags.
///
/// `c` from `start` to `last` by `step` becomes `c - final`, `final` being
/// `last + step`, and the exit test `c - final` against zero. Every other
/// read is an invariant plus the counter, or plus the counter shifted, and
/// the invariant takes `final`, shifted the same, once before the loop:
/// both sides wrap at the add's own width, so the sum is unchanged.
pub(crate) fn zeroed(body: &Rc<MirBody>, address_offsets: bool) -> Result<Rc<MirBody>, SubstitutionError> {
    let facts = consts::known(body, None, None, None, None);
    let blocks = body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect::<BTreeMap<_, _>>();
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let predecessors = loops::predecessors(&body.blocks);
    let operation = |at: (usize, usize)| &body.blocks[at.0].ops[at.1];
    let mut made = BTreeMap::new();
    let mut home = BTreeMap::new();
    let mut readers = BTreeMap::<Value, Vec<(usize, usize)>>::new();
    let mut placed = BTreeMap::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        for (operation_index, op) in block.ops.iter().enumerate() {
            let at = (block_index, operation_index);
            for value in &op.defines {
                made.insert(*value, at);
                home.insert(*value, block.at);
            }
            for value in &op.uses {
                readers.entry(*value).or_default().push(at);
            }
            placed.insert(at, block.at);
        }
    }
    home.extend(body.blocks.iter().flat_map(|block| block.phis.iter().map(move |phi| (phi.result, block.at))));
    let in_phis = body
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .flat_map(|phi| phi.incoming.values().copied())
        .collect::<BTreeSet<_>>();
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        if loop_.latches.len() != 1 {
            continue;
        }
        let preheader = transform::_preheader(body, &loop_);
        let Some(preheader) = preheader.filter(|preheader| body.blocks[blocks[preheader]].succ == [loop_.header])
        else {
            continue;
        };
        let header_index = blocks[&loop_.header];
        let header = &body.blocks[header_index];
        let latch = *loop_.latches.iter().next().expect("one latch");
        if header.ops.last().is_none_or(|op| op.kind != Kind::Branch) {
            continue;
        }
        let branch_index = header.ops.len() - 1;
        let branch = &header.ops[branch_index];
        let inside = loop_.body.clone();
        for counter in induction::basics(body, &loop_).values() {
            let phi = header.phis.iter().find(|phi| phi.result.id == counter.value).expect("a basic counter's phi");
            if phi.incoming.keys().copied().collect::<BTreeSet<_>>() != BTreeSet::from([preheader, latch]) {
                continue;
            }
            let initial = *phi.incoming.get(&preheader).expect("checked incoming");
            let update = *phi.incoming.get(&latch).expect("checked incoming");
            let (seed, stepping_at) = (made.get(&initial).copied().map(operation), made.get(&update).copied());
            let (Some(seed), Some(stepping_at)) = (seed, stepping_at) else {
                continue;
            };
            let stepping = operation(stepping_at);
            let Some(Arg::Held(stepped)) = stepping.results.first() else {
                continue;
            };
            let width = stepped.width;
            let Some(Arg::Const(seeded)) = seed.args.first().filter(|_| seed.kind == Kind::Copy && seed.args.len() == 1)
            else {
                continue;
            };
            let Some(proof) = induction::controlling(body, &loop_, counter, &facts) else {
                continue;
            };
            // Every rebased use is exact modulo the width, so a counter that wraps
            // signed still counts to zero; only a narrower compare needs no wrap.
            let Some(count) = proof.count.clone().filter(|_| !proof.posttested) else {
                continue;
            };
            if proof.last.is_none() && proof.width() != width {
                continue;
            }
            if proof.bound == AffineOperand::Const(Const::new(0, proof.width())) && proof.test == Kind::Ne {
                continue; // counts to zero already
            }
            let (compare_index, compare) = (proof.compare.operation_index(), proof.compare_in(body));
            let start = induction::_signed(&Arg::Const(seeded.clone()), &IndexMap::default(), seeded.width)
                .expect("a constant is known");
            let step = proof.step.clone();
            let final_ = &start + count * &step;
            let offsets = _offsets(
                phi.result,
                &readers,
                &placed,
                &home,
                &inside,
                &BTreeSet::from([(header_index, compare_index), stepping_at]),
                body,
                address_offsets,
            );
            let Some(offsets) = offsets else {
                continue;
            };
            let read = body
                .blocks
                .iter()
                .flat_map(|block| &block.ops)
                .flat_map(|op| op.uses.iter().copied())
                .collect::<BTreeSet<_>>();
            if stepping.defines.iter().any(|value| value.flags && (read.contains(value) || in_phis.contains(value))) {
                continue;
            }
            if body.blocks.iter().flat_map(|block| &block.ops).any(|op| op.uses.contains(&update))
                || read.contains(&initial)
            {
                continue;
            }
            if body.blocks.iter().flat_map(|block| &block.ops).any(|op| {
                compare.defines.iter().any(|value| op.uses.contains(value)) && !std::ptr::eq(op, branch)
            }) {
                continue;
            }
            let [exit_at] = header.succ.iter().copied().filter(|at| !inside.contains(at)).collect::<Vec<_>>()[..]
            else {
                panic!("one exit from the header");
            };
            let exit_block = &body.blocks[blocks[&exit_at]];
            if predecessors.get(&exit_at).cloned().unwrap_or_default() != BTreeSet::from([header.at])
                || exit_block.ops.is_empty()
            {
                continue;
            }
            let mut closed = IndexMap::<Value, &Phi>::default();
            for other in &exit_block.phis {
                if other.incoming.len() == 1 {
                    for (predecessor, value) in other.incoming.iter() {
                        if inside.contains(predecessor) {
                            closed.insert(*value, other);
                        }
                    }
                }
            }
            let following = blocks
                .keys()
                .copied()
                .filter(|at| dominators.get(at).is_some_and(|dominating| dominating.contains(&exit_at)))
                .collect::<BTreeSet<_>>();
            let leaving = transform::_leaving(body);
            if [phi.result, update].iter().any(|value| leaving.contains(value)) {
                continue;
            }
            if body.blocks.iter().flat_map(|block| &block.phis).any(|other| {
                !std::ptr::eq(other, phi)
                    && !closed.values().any(|one| *one == other)
                    && other.incoming.values().any(|value| *value == phi.result || *value == update)
            }) {
                continue;
            }
            if body.blocks.iter().any(|block| {
                block.ops.iter().any(|op| op.uses.contains(&phi.result))
                    && !following.contains(&block.at)
                    && !inside.contains(&block.at)
            }) {
                continue;
            }
            let mut serial = ssa::values(body).map(|value| value.id).max().expect("a value") + 1;
            let mut variable = ssa::values(body).map(|value| value.variable).max().expect("a value") + 1;
            let ending = body.blocks[blocks[&preheader]].ops.last().expect("a preheader operation");
            let mut seeds = Vec::new();
            let mut rebased = BTreeMap::new();
            for (at, position, multiplier, address, extra) in &offsets {
                let op = operation(*at);
                let Some(position) = *position else {
                    let (source, replacement) = address.expect("an address offset");
                    rebased.insert(*at, _rebased_cells(op, source, &(&final_ * multiplier + extra), replacement));
                    continue;
                };
                match &op.args[position] {
                    Arg::Const(base) => {
                        let mut replacement = op.clone();
                        replacement.args[position] = Arg::Const(Const::new(
                            consts::masked(&(&base.n + &final_ * multiplier), base.width),
                            base.width,
                        ));
                        replacement.source_backed = false;
                        replacement.raised = None;
                        rebased.insert(*at, replacement);
                    }
                    Arg::Held(base) => {
                        let moved = Value { id: serial, at: ending.at, flags: false, variable, version: 0 };
                        (serial, variable) = (serial + 1, variable + 1);
                        seeds.push(strength::_made(
                            Kind::Add,
                            "add",
                            moved,
                            vec![
                                Arg::Held(*base),
                                Arg::Const(Const::new(consts::masked(&(&final_ * multiplier), base.width), base.width)),
                            ],
                            ending.at,
                            ending,
                        ));
                        let mut replacement = op.clone();
                        replacement.args[position] = Arg::Held(Held { value: moved, width: base.width });
                        replacement.uses =
                            op.uses.iter().map(|value| if *value == base.value { moved } else { *value }).collect();
                        rebased.insert(*at, replacement);
                    }
                    _ => panic!("assert isinstance(base, mir.Held)"),
                }
            }
            let finished = Value { id: serial, at: exit_at, flags: false, variable, version: 0 };
            // A start of its own: the constant it was seeded from can start another loop too.
            let begun = Value { id: serial + 1, at: ending.at, flags: false, variable: variable + 1, version: 0 };
            seeds.push(strength::_made(
                Kind::Copy,
                "",
                begun,
                vec![Arg::Const(Const::new(consts::masked(&(&start - &final_), seeded.width), seeded.width))],
                ending.at,
                ending,
            ));
            let finish = strength::_made(
                Kind::Copy,
                "",
                finished,
                vec![Arg::Const(Const::new(consts::masked(&final_, width), width))],
                exit_at,
                &exit_block.ops[0],
            );
            let mut swap = BTreeMap::from([(counter.value, finished)]);
            let removed = closed
                .iter()
                .filter(|(value, _other)| **value == phi.result || **value == update)
                .map(|(_value, other)| other.result)
                .collect::<BTreeSet<_>>();
            swap.extend(removed.iter().map(|value| (value.id, finished)));
            let changed = loopexit::_substituted_exits(body, exit_at, &following, &[finish], &swap)?;
            let mut out = Vec::new();
            for (block_index, block) in changed.blocks.iter().enumerate() {
                let kept = !following.contains(&block.at);
                let mut ops = Vec::new();
                for (operation_index, op) in block.ops.iter().enumerate() {
                    let at = (block_index, operation_index);
                    let op = if kept && at == (header_index, compare_index) {
                        let mut op = op.clone();
                        op.args = vec![Arg::Held(Held { value: phi.result, width }), Arg::Const(Const::new(0, width))];
                        op.kind = Kind::Sub;
                        op.results = Vec::new();
                        op.defines.retain(|value| value.flags);
                        op.uses = vec![phi.result];
                        op.loads = Vec::new();
                        op.source_backed = false;
                        op.raised = None;
                        op
                    } else if kept && at == (header_index, branch_index) {
                        let test =
                            if branch.target.is_some_and(|target| inside.contains(&target)) { Kind::Ne } else { Kind::Eq };
                        let mut op = op.clone();
                        op.test = Some(test);
                        op.name.clear();
                        op.source_backed = false;
                        op.raised = Some((Vec::new(), Vec::new()));
                        op
                    } else if kept {
                        rebased.get(&at).cloned().unwrap_or_else(|| op.clone())
                    } else {
                        op.clone()
                    };
                    ops.push(op);
                }
                if block.at == preheader {
                    _before_leaving(&mut ops, seeds.clone());
                }
                let phis = block
                    .phis
                    .iter()
                    .filter(|other| !removed.contains(&other.result))
                    .map(|other| {
                        let mut other = other.clone();
                        if other.result == phi.result {
                            other.incoming.insert(preheader, begun);
                        }
                        other
                    })
                    .collect();
                out.push(MirBlock { at: block.at, phis, ops, succ: block.succ.clone(), cold: block.cold });
            }
            return zeroed(&Rc::new(MirBody { blocks: out, ..changed }), address_offsets);
        }
    }
    Ok(body.clone())
}

/// `(add, invariant's position, multiplier, address, extra)`, the add named by its occurrence.
type _Offset = ((usize, usize), Option<usize>, BigInt, Option<(Value, Value)>, BigInt);

/// Every add of an invariant to the counter, as (add, invariant's position, multiplier).
///
/// Also through a shift of the counter read only by such adds. None where
/// the counter is read any other way inside the loop, or a flag an add or
/// shift sets is read.
#[allow(clippy::too_many_arguments)]
fn _offsets(
    counter: Value,
    readers: &BTreeMap<Value, Vec<(usize, usize)>>,
    placed: &BTreeMap<(usize, usize), i64>,
    home: &BTreeMap<Value, i64>,
    inside: &BTreeSet<i64>,
    own: &BTreeSet<(usize, usize)>,
    body: &MirBody,
    address_offsets: bool,
) -> Option<Vec<_Offset>> {
    let operation = |at: (usize, usize)| &body.blocks[at.0].ops[at.1];
    let flagless = |op: &Op| !op.defines.iter().any(|value| value.flags && readers.contains_key(value));
    let plain = |op: &Op, kind: Kind| {
        op.kind == kind
            && !(!op.loads.is_empty() || !op.stores.is_empty() || !op.merges.is_empty() || op.barrier())
            && op.results.len() == 1
            && matches!(op.results[0], Arg::Held(_))
            && flagless(op)
    };
    let result_width = |op: &Op| match &op.results[0] {
        Arg::Held(held) => held.width,
        _ => unreachable!("plain checked the sole result"),
    };

    let added = |at: (usize, usize), value: Value, multiplier: BigInt| -> Option<_Offset> {
        let op = operation(at);
        if !plain(op, Kind::Add) || op.args.len() != 2 {
            return None;
        }
        let width = result_width(op);
        if !op.args.iter().all(|arg| match arg {
            Arg::Held(held) => held.width == width,
            Arg::Const(constant) => address_offsets && constant.width == width,
            _ => false,
        }) {
            return None;
        }
        let counted = op
            .args
            .iter()
            .enumerate()
            .filter(|(_, arg)| matches!(arg, Arg::Held(held) if held.value == value))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if counted.len() != 1 {
            return None;
        }
        let position = 1 - counted[0];
        if let Arg::Held(invariant) = &op.args[position] {
            if home.get(&invariant.value).is_some_and(|at| inside.contains(at)) {
                return None;
            }
        }
        Some((at, Some(position), multiplier, None, BigInt::from(0_u8)))
    };

    let addressed = |at: (usize, usize),
                     value: Value,
                     multiplier: BigInt,
                     replacement: Option<Value>,
                     extra: BigInt|
     -> Option<_Offset> {
        let op = operation(at);
        let refs = op
            .loads
            .iter()
            .chain(&op.stores)
            .chain(op.memory_values.iter().map(|(reference, _known)| reference))
            .collect::<Vec<&MemRef>>();
        let found = refs.iter().copied().filter(|reference| reference.base == Some(value)).collect::<Vec<_>>();
        if found.is_empty()
            || found.iter().any(|reference| reference.addr.is_none() || reference.symbolic.is_some())
            || op.args.iter().any(|arg| matches!(arg, Arg::Held(held) if held.value == value))
            || refs.iter().any(|reference| reference.segment == Some(value))
            || refs.iter().any(|reference| !found.contains(reference) && reference.base == Some(value))
        {
            return None;
        }
        Some((at, None, multiplier, Some((value, replacement.unwrap_or(value))), extra))
    };

    let derived = |at: (usize, usize), value: Value, multiplier: BigInt| {
        let form = added(at, value, multiplier.clone());
        if form.is_some() || !address_offsets {
            return form;
        }
        addressed(at, value, multiplier, None, BigInt::from(0_u8))
    };

    // The invariant side of an equality, shifted with the counter.
    //
    // Replacing `counter` by `counter - final` preserves identity only
    // when the other side becomes `other - final` too.  This is safe for
    // equality and inequality flags; ordered comparisons would change.
    let equality = |at: (usize, usize), value: Value| -> Option<_Offset> {
        let op = operation(at);
        if !address_offsets
            || op.kind != Kind::Sub
            || !op.results.is_empty()
            || !op.loads.is_empty()
            || !op.stores.is_empty()
            || op.barrier()
            || !op.merges.is_empty()
            || op.args.len() != 2
            || op.defines.len() != 1
            || !op.defines[0].flags
            || readers.get(&op.defines[0]).is_none_or(|users| users.is_empty())
            || readers[&op.defines[0]].iter().any(|reader| {
                let reader = operation(*reader);
                reader.kind != Kind::Branch || !matches!(reader.test, Some(Kind::Eq | Kind::Ne))
            })
        {
            return None;
        }
        let positions = op
            .args
            .iter()
            .enumerate()
            .filter(|(_, arg)| matches!(arg, Arg::Held(held) if held.value == value))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if positions.len() != 1 {
            return None;
        }
        let position = 1 - positions[0];
        let Arg::Held(counted) = &op.args[positions[0]] else {
            unreachable!("positions are held");
        };
        match &op.args[position] {
            Arg::Held(other) if other.width == counted.width && !home.get(&other.value).is_some_and(|at| inside.contains(at)) => {}
            Arg::Const(other) if other.width == counted.width => {}
            _ => return None,
        }
        Some((at, Some(position), BigInt::from(-1_i8), None, BigInt::from(0_u8)))
    };

    let mut out = Vec::new();
    for at in readers.get(&counter).into_iter().flatten().copied() {
        if own.contains(&at) || !inside.contains(&placed[&at]) {
            continue;
        }
        let op = operation(at);
        let mut form = if address_offsets { addressed(at, counter, BigInt::from(1_u8), None, BigInt::from(0_u8)) } else { None };
        let added_form = added(at, counter, BigInt::from(1_u8));
        if address_offsets {
            if let Some((_, Some(position), _, _, _)) = &added_form {
                if let Arg::Const(invariant) = &op.args[*position] {
                    let Arg::Held(result) = &op.results[0] else {
                        unreachable!("added checked the result");
                    };
                    let result = result.value;
                    let constant = induction::_signed(&Arg::Const(invariant.clone()), &IndexMap::default(), invariant.width)
                        .expect("a constant is known");
                    let forms = readers
                        .get(&result)
                        .into_iter()
                        .flatten()
                        .map(|reader| addressed(*reader, result, BigInt::from(1_u8), Some(counter), constant.clone()))
                        .collect::<Vec<_>>();
                    if !forms.is_empty() && forms.iter().all(Option::is_some) {
                        out.extend(forms.into_iter().flatten());
                        continue;
                    }
                }
            }
        }
        if form.is_none() {
            form = added_form;
        }
        if form.is_none() {
            form = equality(at, counter);
        }
        let mut scale = None;
        if plain(op, Kind::Shl) && op.args.len() == 2 && matches!(op.args[1], Arg::Const(_)) {
            if op.args[0] == Arg::Held(Held { value: counter, width: result_width(op) }) {
                let Arg::Const(shift) = &op.args[1] else {
                    unreachable!("checked constant");
                };
                scale = Some(BigInt::from(1_u8) << usize::try_from(&shift.n).expect("negative shift count"));
            }
        } else if address_offsets && plain(op, Kind::Mul) && op.args.len() == 2 {
            let constants = op
                .args
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Const(constant) => Some(constant),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let held = op
                .args
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Held(held) if held.value == counter => Some(held),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if constants.len() == 1
                && held.len() == 1
                && constants[0].width == held[0].width
                && held[0].width == result_width(op)
            {
                scale = induction::_signed(&Arg::Const(constants[0].clone()), &IndexMap::default(), constants[0].width);
            }
        }
        if form.is_none() {
            if let Some(scale) = &scale {
                let Arg::Held(shifted) = &op.results[0] else {
                    unreachable!("plain checked the result");
                };
                let shifted = shifted.value;
                let forms = readers
                    .get(&shifted)
                    .into_iter()
                    .flatten()
                    .map(|reader| derived(*reader, shifted, scale.clone()))
                    .collect::<Vec<_>>();
                if !forms.is_empty() && forms.iter().all(Option::is_some) {
                    out.extend(forms.into_iter().flatten());
                    continue;
                }
            }
        }
        out.push(form?);
    }
    Some(out)
}

/// Move the fixed part of every address using `base` by displacement.
fn _rebased_cells(op: &Op, base: Value, displacement: &BigInt, replacement: Value) -> Op {
    let displacement = i64::try_from(displacement).expect("a displacement fits an address");
    let reference = |reference: &MemRef| -> MemRef {
        if reference.base != Some(base) {
            return reference.clone();
        }
        assert!(reference.addr.is_some() && reference.symbolic.is_none());
        let mut moved = reference.clone();
        moved.addr = reference.addr.as_ref().map(|addr| addr.plus(displacement));
        moved.base = Some(replacement);
        moved
    };
    let operand = |arg: &Arg| match arg {
        Arg::Cell(cell) => Arg::Cell(mir::Cell { r#ref: reference(&cell.r#ref) }),
        _ => arg.clone(),
    };
    let mut rebased = op.clone();
    rebased.args = op.args.iter().map(operand).collect();
    rebased.results = op.results.iter().map(operand).collect();
    rebased.loads = op.loads.iter().map(reference).collect();
    rebased.stores = op.stores.iter().map(reference).collect();
    rebased.memory_values = op.memory_values.iter().map(|(one, known)| (reference(one), known.clone())).collect();
    rebased.uses = op.uses.iter().map(|value| if *value == base { replacement } else { *value }).collect();
    rebased.source_backed = false;
    rebased.raised = None;
    rebased
}

#[cfg(test)]
#[path = "indvars_tests.rs"]
mod tests;
