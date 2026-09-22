//! Evaluate affine exit values and delete finite, side-effect-free counted loops.
//!
//! Direct port of `qbopt/optimize/loopexit.py`.  The integer equivalent of
//! evaluating an AddRec at its backedge count: a fixed increment sums to
//! N * step; an affine increment also contributes N(N-1)/2 times its stride.
//! Results are modulo their own width.  Only the controlling recurrence must
//! be proven not to wrap.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;
use num_bigint::BigInt;

use crate::analysis::consts::{self, Known};
use crate::analysis::induction::{self, Affine, AffineOperand};
use crate::analysis::loops::{self, Loop};
use crate::analysis::{ranges, ssa};
use crate::model::mir::{Arg, Const, Held, Kind, MirBlock, MirBody, Op, OrderedMap, Value};

use super::{strength, transform};

/// Python's `list[tuple[mir.Held | mir.Const, int]]` exit terms.
type Terms = Vec<(AffineOperand, BigInt)>;

/// Python `evaluated`.  Python returns `body` itself when nothing changes;
/// Rust returns an equal clone.
pub(crate) fn evaluated(body: &Rc<MirBody>) -> Result<Rc<MirBody>, String> {
    let facts = consts::known(body, None, None, None, None);
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        if loop_.body.len() != 2 || loop_.latches.len() != 1 {
            continue;
        }
        let header = blocks[&loop_.header];
        let latch = blocks[loop_.latches.iter().next().expect("one latch")];
        let preheader = transform::_preheader(body, &loop_);
        let Some(preheader) = preheader else {
            continue;
        };
        if blocks[&preheader].succ != [header.at] || !latch.phis.is_empty() {
            continue;
        }
        let counters = induction::basics(body, &loop_);
        if counters.is_empty() {
            continue;
        }
        let mut counts = BTreeSet::new();
        for counter in counters.values() {
            let width = counter.start.width();
            let last = induction::_last_counter(body, &loop_, counter, &facts, width);
            let start = induction::_signed(&counter.start.as_arg(), &facts, width);
            let step = induction::_signed(&counter.step.as_arg(), &facts, width);
            if let (Some(last), Some(start), Some(step)) = (last, start, step) {
                if step != BigInt::from(0) {
                    counts.insert(induction::floor_div(&(last - start), &step) + 1);
                }
            }
        }
        if counts.len() != 1 {
            continue;
        }
        let count = counts.pop_first().expect("one count");
        let exits = _exit_terms(body, &loop_, &counters, &count, &facts)?;
        if exits.is_empty() {
            continue;
        }
        let exit_at = match header
            .succ
            .iter()
            .copied()
            .filter(|at| !loop_.body.contains(at))
            .collect::<Vec<_>>()[..]
        {
            [one] => one,
            [] => return Err("not enough values to unpack (expected 1, got 0)".to_string()),
            _ => return Err("too many values to unpack (expected 1)".to_string()),
        };
        let exited = exits.keys().copied().collect::<BTreeSet<_>>();
        let phied = header
            .phis
            .iter()
            .map(|phi| phi.result.id)
            .collect::<BTreeSet<_>>();
        if exited != phied || !_disposable(body, &loop_, header, latch) {
            let changed = _constant_exits(body, &loop_, &exits, exit_at, &facts)?;
            if let Some(changed) = changed {
                return Ok(changed);
            }
            continue;
        }
        let mut serial = ssa::values(body)
            .map(|value| value.id)
            .max()
            .expect("a value")
            + 1;
        let mut variable = ssa::values(body)
            .map(|value| value.variable)
            .max()
            .expect("a value")
            + 1;
        let mut calculations = Vec::new();
        for phi in &header.phis {
            let terms = &exits[&phi.result.id];
            let width = terms[0].0.width();
            let mut total = Arg::Const(Const::new(0, width));
            for (arg, coefficient) in terms {
                let product = if let AffineOperand::Const(arg) = arg {
                    Arg::Const(Const::new(
                        consts::masked(&(&arg.n * coefficient), width),
                        width,
                    ))
                } else if *coefficient == BigInt::from(1) {
                    arg.as_arg()
                } else {
                    let temporary = Value {
                        id: serial,
                        at: header.at,
                        flags: false,
                        variable,
                        version: 0,
                    };
                    serial += 1;
                    variable += 1;
                    calculations.push(strength::_made(
                        Kind::Mul,
                        "",
                        temporary,
                        vec![
                            arg.as_arg(),
                            Arg::Const(Const::new(consts::masked(coefficient, width), width)),
                        ],
                        header.at,
                        &header.ops[0],
                    ));
                    Arg::Held(Held {
                        value: temporary,
                        width,
                    })
                };
                let temporary = Value {
                    id: serial,
                    at: header.at,
                    flags: false,
                    variable,
                    version: 0,
                };
                serial += 1;
                variable += 1;
                calculations.push(strength::_made(
                    Kind::Add,
                    "",
                    temporary,
                    vec![total, product],
                    header.at,
                    &header.ops[0],
                ));
                total = Arg::Held(Held {
                    value: temporary,
                    width,
                });
            }
            calculations.push(strength::_made(
                Kind::Copy,
                "",
                phi.result,
                vec![total],
                header.at,
                &header.ops[0],
            ));
        }
        let mut jump = header.ops[header.ops.len() - 1].clone();
        jump.kind = Kind::Jump;
        jump.uses = Vec::new();
        jump.args = Vec::new();
        jump.results = Vec::new();
        jump.target = Some(exit_at);
        jump.raised = Some((Vec::new(), Vec::new()));
        jump.test = None;
        // Preserve all original byte ownership while replacing the header's work.
        let mut replacement = header.clone();
        replacement.phis = Vec::new();
        replacement.succ = vec![exit_at];
        replacement.ops = calculations
            .into_iter()
            .chain(header.ops[..header.ops.len() - 1].iter().map(_cleared))
            .chain(std::iter::once(jump))
            .collect();
        let mut changed = MirBody::clone(body);
        changed.blocks = body
            .blocks
            .iter()
            .map(|block| {
                if block.at == header.at {
                    replacement.clone()
                } else if block.at == latch.at {
                    let mut cleared = block.clone();
                    cleared.succ = Vec::new();
                    cleared.ops = block.ops.iter().map(_cleared).collect();
                    cleared
                } else {
                    block.clone()
                }
            })
            .collect();
        return transform::_trivial_phis(&transform::_unreachable(&changed))
            .map(Rc::new)
            .map_err(|error| error.to_string());
    }
    Ok(body.clone())
}

/// Python `_exit_terms`: sum a linear increment over N iterations using
/// N(N-1)/2, before modular reduction.
fn _exit_terms(
    body: &Rc<MirBody>,
    loop_: &Loop,
    counters: &OrderedMap<u32, Affine>,
    count: &BigInt,
    facts: &IndexMap<Value, Known>,
) -> Result<IndexMap<u32, Terms>, String> {
    let header = body
        .blocks
        .iter()
        .find(|block| block.at == loop_.header)
        .expect("the loop header");
    let made = body
        .blocks
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .flat_map(|block| block.ops.iter())
        .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
        .collect::<BTreeMap<Value, &Op>>();
    let still = induction::invariant(body, &loop_.body);
    let mut headers = header
        .phis
        .iter()
        .map(|phi| phi.result)
        .collect::<BTreeSet<_>>();
    let widened = _widened_counters(body, loop_, counters, facts)?;
    headers.extend(widened.keys().copied());
    let mut merged = counters
        .iter()
        .map(|(value, counter)| (*value, counter.clone()))
        .collect::<IndexMap<_, _>>();
    merged.extend(
        widened
            .iter()
            .map(|(value, counter)| (value.id, counter.clone())),
    );
    let counters = merged;
    let mut exits = IndexMap::new();
    for phi in &header.phis {
        if let Some(counter) = counters.get(&phi.result.id) {
            exits.insert(
                phi.result.id,
                vec![
                    (counter.start.clone(), BigInt::from(1)),
                    (counter.step.clone(), count.clone()),
                ],
            );
            continue;
        }
        let start = match phi
            .incoming
            .iter()
            .filter(|(at, _)| !loop_.body.contains(at))
            .map(|(_, value)| *value)
            .collect::<Vec<_>>()[..]
        {
            [one] => one,
            [] => return Err("not enough values to unpack (expected 1, got 0)".to_string()),
            _ => return Err("too many values to unpack (expected 1)".to_string()),
        };
        let update = match phi
            .incoming
            .iter()
            .filter(|(at, _)| loop_.body.contains(at))
            .map(|(_, value)| *value)
            .collect::<Vec<_>>()[..]
        {
            [one] => one,
            [] => return Err("not enough values to unpack (expected 1, got 0)".to_string()),
            _ => return Err("too many values to unpack (expected 1)".to_string()),
        };
        let Some(op) = made.get(&update) else {
            continue;
        };
        let width = match op.results[..] {
            [Arg::Held(held)] => held.width,
            _ => continue,
        };
        let linear = _linear(
            &Arg::Held(Held {
                value: update,
                width,
            }),
            &made,
            &headers,
            width,
            &BTreeSet::new(),
            &mut IndexMap::new(),
        );
        let Some(mut linear) = linear else {
            continue;
        };
        let own = linear
            .shift_remove(&Arg::Held(Held {
                value: phi.result,
                width,
            }))
            .unwrap_or_else(|| BigInt::from(0));
        if own != BigInt::from(1) {
            continue;
        }
        let mut terms = vec![(
            AffineOperand::Held(Held {
                value: start,
                width,
            }),
            BigInt::from(1),
        )];
        let mut complete = true;
        for (arg, coefficient) in &linear {
            match arg {
                Arg::Const(constant) => {
                    terms.push((AffineOperand::Const(constant.clone()), coefficient * count));
                }
                Arg::Held(held) if still.contains(&held.value.id) => {
                    terms.push((AffineOperand::Held(*held), coefficient * count));
                }
                Arg::Held(held)
                    if counters
                        .get(&held.value.id)
                        .is_some_and(|counter| counter.start.width() == width) =>
                {
                    let counter = &counters[&held.value.id];
                    terms.push((counter.start.clone(), coefficient * count));
                    terms.push((
                        counter.step.clone(),
                        induction::floor_div(
                            &(coefficient * count * (count - 1)),
                            &BigInt::from(2),
                        ),
                    ));
                }
                _ => {
                    complete = false;
                    break;
                }
            }
        }
        if complete {
            exits.insert(phi.result.id, terms);
        }
    }
    Ok(exits)
}

/// Python `_widened_counters`.
fn _widened_counters(
    body: &Rc<MirBody>,
    loop_: &Loop,
    counters: &OrderedMap<u32, Affine>,
    facts: &IndexMap<Value, Known>,
) -> Result<IndexMap<Value, Affine>, String> {
    if !body
        .blocks
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .flat_map(|block| block.ops.iter())
        .any(|op| op.kind == Kind::SignExtend)
    {
        return Ok(IndexMap::new());
    }
    let bounds = ranges::bounded(body)?;
    let mut widened = IndexMap::new();
    for block in &body.blocks {
        if !loop_.body.contains(&block.at) {
            continue;
        }
        for op in &block.ops {
            if op.kind != Kind::SignExtend
                || op.args.len() != op.results.len()
                || op.args.len() != 1
                || !op.loads.is_empty()
                || !op.stores.is_empty()
                || !op.merges.is_empty()
            {
                continue;
            }
            let (Arg::Held(source), Arg::Held(result)) = (&op.args[0], &op.results[0]) else {
                continue;
            };
            if source.width >= result.width {
                continue;
            }
            let counter = counters.get(&source.value.id);
            let interval = bounds
                .get(&block.at)
                .and_then(|known| known.get(&source.value));
            let (Some(counter), Some(interval)) = (counter, interval) else {
                continue;
            };
            if interval.width != source.width {
                continue;
            }
            let start = induction::_signed(&counter.start.as_arg(), facts, source.width);
            let step = induction::_signed(&counter.step.as_arg(), facts, source.width);
            if let (Some(start), Some(step)) = (start, step) {
                widened.insert(
                    result.value,
                    Affine {
                        value: result.value.id,
                        start: AffineOperand::Const(Const::new(start, result.width)),
                        step: AffineOperand::Const(Const::new(step, result.width)),
                        header: loop_.header,
                    },
                );
            }
        }
    }
    Ok(widened)
}

/// Python `_constant_exits`: replace constant live-outs after the loop,
/// leaving its observable work intact.  `None` is Python returning `body`.
fn _constant_exits(
    body: &Rc<MirBody>,
    loop_: &Loop,
    exits: &IndexMap<u32, Terms>,
    exit_at: i64,
    facts: &IndexMap<Value, Known>,
) -> Result<Option<Rc<MirBody>>, String> {
    let predecessors = loops::predecessors(&body.blocks);
    if predecessors.get(&exit_at).cloned().unwrap_or_default() != BTreeSet::from([loop_.header]) {
        return Ok(None);
    }
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let following = body
        .blocks
        .iter()
        .filter(|block| {
            dominators
                .get(&block.at)
                .is_some_and(|dominating| dominating.contains(&exit_at))
        })
        .map(|block| block.at)
        .collect::<BTreeSet<_>>();
    let mut used = body
        .blocks
        .iter()
        .filter(|block| following.contains(&block.at))
        .flat_map(|block| block.ops.iter())
        .flat_map(|op| op.uses.iter().map(|value| value.id))
        .collect::<BTreeSet<_>>();
    used.extend(
        body.blocks
            .iter()
            .flat_map(|block| block.phis.iter())
            .flat_map(|phi| phi.incoming.iter())
            .filter(|(predecessor, _)| following.contains(predecessor))
            .map(|(_, value)| value.id),
    );
    let mut serial = ssa::values(body)
        .map(|value| value.id)
        .max()
        .expect("a value")
        + 1;
    let mut variable = ssa::values(body)
        .map(|value| value.variable)
        .max()
        .expect("a value")
        + 1;
    let exit_block = body
        .blocks
        .iter()
        .find(|block| block.at == exit_at)
        .expect("the exit block");
    if exit_block.ops.is_empty() {
        return Ok(None);
    }
    let aliases = exit_block
        .phis
        .iter()
        .filter(|phi| {
            phi.incoming.keys().copied().collect::<BTreeSet<_>>() == BTreeSet::from([loop_.header])
        })
        .map(|phi| {
            (
                phi.result.id,
                phi.incoming.get(&loop_.header).expect("header edge").id,
            )
        })
        .collect::<IndexMap<u32, u32>>();
    let mut unavailable = body
        .blocks
        .iter()
        .filter(|block| !following.contains(&block.at))
        .flat_map(|block| block.ops.iter())
        .flat_map(|op| op.uses.iter())
        .filter_map(|value| aliases.get(&value.id).copied())
        .collect::<BTreeSet<_>>();
    unavailable.extend(
        body.blocks
            .iter()
            .flat_map(|block| block.phis.iter())
            .flat_map(|phi| phi.incoming.iter())
            .filter(|(predecessor, _)| !following.contains(predecessor))
            .filter_map(|(_, value)| aliases.get(&value.id).copied()),
    );
    let aliased = used
        .iter()
        .filter_map(|value| aliases.get(value).copied())
        .collect::<Vec<_>>();
    used.extend(aliased);
    let mut added = Vec::new();
    let mut swap = IndexMap::<u32, Value>::new();
    for (value, terms) in exits {
        if !used.contains(value) || unavailable.contains(value) {
            continue;
        }
        let width = terms[0].0.width();
        let mut total = BigInt::from(0);
        let mut complete = true;
        for (arg, coefficient) in terms {
            let fact = match arg {
                AffineOperand::Held(held) => facts.get(&held.value).cloned(),
                AffineOperand::Const(constant) => {
                    Some(Known::new(constant.n.clone(), constant.width))
                }
            };
            let Some(fact) = fact.filter(|fact| fact.width >= arg.width()) else {
                complete = false;
                break;
            };
            total += consts::masked(&fact.n, arg.width()) * coefficient;
        }
        if complete {
            let result = Value {
                id: serial,
                at: exit_at,
                flags: false,
                variable,
                version: 0,
            };
            serial += 1;
            variable += 1;
            added.push(strength::_made(
                Kind::Copy,
                "",
                result,
                vec![Arg::Const(Const::new(consts::masked(&total, width), width))],
                exit_at,
                &exit_block.ops[0],
            ));
            swap.insert(*value, result);
        }
    }
    if swap.is_empty() {
        return Ok(None);
    }

    let with_aliases = |replacements: &IndexMap<u32, Value>| -> BTreeMap<u32, Value> {
        let mut merged = replacements
            .iter()
            .map(|(value, result)| (*value, *result))
            .collect::<BTreeMap<_, _>>();
        merged.extend(aliases.iter().filter_map(|(alias, source)| {
            replacements.get(source).map(|result| (*alias, *result))
        }));
        merged
    };

    let changed = Rc::new(
        _substituted_exits(body, exit_at, &following, &added, &with_aliases(&swap)).map_err(|error| error.to_string())?,
    );
    let alive = transform::live(&changed)
        .into_iter()
        .map(|value| value.id)
        .collect::<BTreeSet<_>>();
    let profitable = swap
        .iter()
        .filter(|(value, _)| !alive.contains(value))
        .map(|(value, result)| (*value, *result))
        .collect::<IndexMap<_, _>>();
    if profitable.is_empty() {
        return Ok(None);
    }
    if profitable == swap {
        return Ok(Some(changed));
    }
    let added = added
        .into_iter()
        .filter(|op| profitable.values().any(|result| *result == op.defines[0]))
        .collect::<Vec<_>>();
    _substituted_exits(
        body,
        exit_at,
        &following,
        &added,
        &with_aliases(&profitable),
    )
    .map(|changed| Some(Rc::new(changed)))
    .map_err(|error| error.to_string())
}

/// Python `_substituted_exits`.
pub(crate) fn _substituted_exits(
    body: &MirBody,
    exit_at: i64,
    following: &BTreeSet<i64>,
    added: &[Op],
    swap: &BTreeMap<u32, Value>,
) -> Result<MirBody, ssa::SubstitutionError> {
    let mut result = body.clone();
    result.blocks = body
        .blocks
        .iter()
        .map(|block| -> Result<MirBlock, ssa::SubstitutionError> {
            let mut replaced = block.clone();
            replaced.phis = block
                .phis
                .iter()
                .filter(|phi| block.at != exit_at || !swap.contains_key(&phi.result.id))
                .map(|phi| {
                    let mut phi = phi.clone();
                    phi.incoming = phi
                        .incoming
                        .iter()
                        .map(|(predecessor, value)| {
                            Ok((
                                *predecessor,
                                if following.contains(predecessor) {
                                    ssa::provider(*value, swap)?
                                } else {
                                    *value
                                },
                            ))
                        })
                        .collect::<Result<_, ssa::SubstitutionError>>()?;
                    Ok(phi)
                })
                .collect::<Result<_, _>>()?;
            let mut ops = if block.at == exit_at {
                added.to_vec()
            } else {
                Vec::new()
            };
            if following.contains(&block.at) {
                for op in &block.ops {
                    ops.push(ssa::substituted(op, swap)?);
                }
            } else {
                ops.extend(block.ops.iter().cloned());
            }
            replaced.ops = ops;
            Ok(replaced)
        })
        .collect::<Result<_, _>>()?;
    Ok(result)
}

/// Python `_linear`.
fn _linear(
    arg: &Arg,
    made: &BTreeMap<Value, &Op>,
    headers: &BTreeSet<Value>,
    width: u32,
    visiting: &BTreeSet<Value>,
    cached: &mut IndexMap<Arg, IndexMap<Arg, BigInt>>,
) -> Option<IndexMap<Arg, BigInt>> {
    let held = match arg {
        Arg::Const(constant) if constant.width == width => None,
        Arg::Held(held) if held.width == width => Some(held),
        _ => return None,
    };
    let Some(held) =
        held.filter(|held| !headers.contains(&held.value) && made.contains_key(&held.value))
    else {
        return Some(IndexMap::from([(arg.clone(), BigInt::from(1))]));
    };
    if visiting.contains(&held.value) {
        return None;
    }
    if let Some(found) = cached.get(arg) {
        return Some(found.clone());
    }
    let op = made[&held.value];
    // Not `op.merges`. That is the two-address tie -- which use shares a
    // register with which definition -- and it says nothing about whether
    // the operation is a linear function of its own arguments. BC writes
    // every accumulator as a two-address `add`, so refusing on it refused
    // every accumulator there is: hotlpx's `s = s + (n*k) + i` linearised
    // to None, so the sum had no exit value and the loop could not go.
    if op.results != [arg.clone()] || !op.loads.is_empty() || !op.stores.is_empty() {
        return None;
    }
    let parts = if op.kind == Kind::Copy && op.args.len() == 1 {
        vec![(op.args[0].clone(), BigInt::from(1))]
    } else if matches!(op.kind, Kind::Add | Kind::Sub) && op.args.len() == 2 {
        vec![
            (op.args[0].clone(), BigInt::from(1)),
            (
                op.args[1].clone(),
                BigInt::from(if op.kind == Kind::Sub { -1 } else { 1 }),
            ),
        ]
    } else if matches!(op.kind, Kind::Increment | Kind::Decrement) && op.args.len() == 1 {
        vec![
            (op.args[0].clone(), BigInt::from(1)),
            (
                Arg::Const(Const::new(1, width)),
                BigInt::from(if op.kind == Kind::Decrement { -1 } else { 1 }),
            ),
        ]
    } else {
        return None;
    };
    let mut result = IndexMap::<Arg, BigInt>::new();
    let mut deeper = visiting.clone();
    deeper.insert(held.value);
    for (source, coefficient) in parts {
        let terms = _linear(&source, made, headers, width, &deeper, cached)?;
        for (term, factor) in terms {
            let entry = result.entry(term).or_insert_with(|| BigInt::from(0));
            *entry += &coefficient * factor;
        }
    }
    let kept = result
        .into_iter()
        .filter(|(_, factor)| *factor != BigInt::from(0))
        .collect::<IndexMap<_, _>>();
    cached.insert(arg.clone(), kept.clone());
    Some(kept)
}

/// Python `_cleared`.
fn _cleared(op: &Op) -> Op {
    let mut result = op.clone();
    result.kind = Kind::Nothing;
    result.name = String::new();
    result.defines = Vec::new();
    result.uses = Vec::new();
    result.loads = Vec::new();
    result.stores = Vec::new();
    result.args = Vec::new();
    result.results = Vec::new();
    result.merges = OrderedMap::new();
    result.raised = None;
    result.target = None;
    result.test = None;
    result.stack = None;
    result.symbol = Some(false);
    result
}

/// Python `_disposable`.
fn _disposable(body: &MirBody, loop_: &Loop, header: &MirBlock, latch: &MirBlock) -> bool {
    let allowed = [
        Kind::Nothing,
        Kind::Copy,
        Kind::Add,
        Kind::Sub,
        Kind::Increment,
        Kind::Decrement,
    ];
    for block in [header, latch] {
        for op in &block.ops {
            // `merges` again: deleting the block removes the tied use and
            // the tied definition together, so the tie cannot outlive it.
            if !op.loads.is_empty() || !op.stores.is_empty() || op.stack.is_some() {
                return false;
            }
            if std::ptr::eq(block, header)
                && header.ops.last().is_some_and(|last| std::ptr::eq(op, last))
            {
                continue; // The trip-count proof checked this branch.
            }
            if !allowed.contains(&op.kind)
                || op
                    .args
                    .iter()
                    .any(|arg| !matches!(arg, Arg::Held(_) | Arg::Const(_)))
            {
                return false;
            }
        }
    }
    let internal = [header, latch]
        .into_iter()
        .flat_map(|block| block.ops.iter())
        .flat_map(|op| op.defines.iter().copied())
        .collect::<BTreeSet<_>>();
    for block in &body.blocks {
        if loop_.body.contains(&block.at) {
            continue;
        }
        if block
            .ops
            .iter()
            .any(|op| op.uses.iter().any(|value| internal.contains(value)))
        {
            return false;
        }
        if block
            .phis
            .iter()
            .any(|phi| phi.incoming.values().any(|value| internal.contains(value)))
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
#[path = "loopexit_tests.rs"]
mod tests;
