//! Sparse value propagation with distinct pending and overdefined states.
//!
//! Port of `qbopt/analysis/constant_cycles.py`.  An optional successor
//! evaluator discovers executable edges.  Pending values are never
//! interpreted as LLVM undef: unresolved reachable values become overdefined
//! before the final result, and unresolved branches retain every successor.

use std::collections::{BTreeSet, VecDeque};

use crate::support::hash::IndexMap;

use super::consts::{self, Known};
use crate::model::mir::{Arg, Kind, MirBlock, MirBody, Op, Phi, Value};
use crate::support::pyset::PySet;

/// Python's `State | Known`: a value's lattice cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum State {
    Pending,
    Overdefined,
    Known(Known),
}

/// The successor evaluator: `None` defers the block's branch.
pub(crate) type Successors<'a> =
    &'a dyn Fn(&MirBlock, &IndexMap<Value, Known>, &IndexMap<Value, State>) -> Option<Vec<i64>>;

fn _meet(first: State, second: State) -> State {
    if first == State::Pending {
        return second;
    }
    if second == State::Pending || first == second {
        return first;
    }
    State::Overdefined
}

enum Recipe<'a> {
    Phi(&'a Phi),
    Op(&'a Op),
}

pub(crate) fn propagated(
    body: &MirBody,
    seeds: &IndexMap<Value, Known>,
    successors: Option<Successors<'_>>,
) -> IndexMap<Value, Known> {
    let mut recipes = IndexMap::default();
    for block in &body.blocks {
        for phi in &block.phis {
            recipes.insert(phi.result, Recipe::Phi(phi));
        }
    }
    for block in &body.blocks {
        for op in &block.ops {
            if let Some(value) = consts::_defined(op) {
                recipes.insert(value, Recipe::Op(op));
            }
        }
    }
    let mut states = recipes
        .keys()
        .map(|value| {
            (
                *value,
                seeds.get(value).cloned().map_or(State::Pending, State::Known),
            )
        })
        .collect::<IndexMap<_, _>>();
    for (value, fact) in seeds {
        states.insert(*value, State::Known(fact.clone()));
    }
    let mut consumers = IndexMap::<Value, PySet<Value>>::default();
    for (value, recipe) in &recipes {
        let inputs = match recipe {
            Recipe::Phi(phi) => phi.incoming.values().copied().collect::<Vec<_>>(),
            Recipe::Op(op) => op
                .args
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Held(held) => Some(held.value),
                    _ => None,
                })
                .collect(),
        };
        for incoming in inputs {
            consumers.entry(incoming).or_default().add(*value);
        }
    }
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<IndexMap<_, _>>();
    let mut owners = IndexMap::default();
    for block in &body.blocks {
        for phi in &block.phis {
            owners.insert(phi.result, block.at);
        }
    }
    for block in &body.blocks {
        for op in &block.ops {
            if let Some(value) = consts::_defined(op) {
                owners.insert(value, block.at);
            }
        }
    }
    let mut live = match successors {
        None => blocks.keys().copied().collect::<PySet<i64>>(),
        Some(_) => [body.entry].into_iter().collect(),
    };
    let mut edges = BTreeSet::<(i64, i64)>::new();
    let mut pending = recipes
        .keys()
        .copied()
        .filter(|value| live.contains(&owners[value]))
        .collect::<VecDeque<_>>();
    let mut queued = pending.iter().copied().collect::<BTreeSet<_>>();

    let enqueue = |values: Vec<Value>,
                   live: &PySet<i64>,
                   pending: &mut VecDeque<Value>,
                   queued: &mut BTreeSet<Value>| {
        for value in values {
            if !queued.contains(&value) && live.contains(&owners[&value]) {
                pending.push_back(value);
                queued.insert(value);
            }
        }
    };
    let consumers_of = |value: &Value| {
        consumers
            .get(value)
            .map(|users| users.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default()
    };

    let activate = |source: i64,
                    target: i64,
                    live: &mut PySet<i64>,
                    edges: &mut BTreeSet<(i64, i64)>,
                    pending: &mut VecDeque<Value>,
                    queued: &mut BTreeSet<Value>| {
        if !blocks.contains_key(&target) || edges.contains(&(source, target)) {
            return false;
        }
        edges.insert((source, target));
        if !live.contains(&target) {
            live.add(target);
            let values = recipes.keys().copied().filter(|value| owners[value] == target).collect();
            enqueue(values, live, pending, queued);
        } else {
            let values = blocks[&target].phis.iter().map(|phi| phi.result).collect();
            enqueue(values, live, pending, queued);
        }
        true
    };
    let supported = |kind: Kind| {
        consts::ARITH.iter().any(|(one, _)| *one == kind)
            || consts::UNARY.iter().any(|(one, _)| *one == kind)
            || matches!(kind, Kind::Copy | Kind::Extract | Kind::SignExtend | Kind::Concat | Kind::Smulhi)
    };
    let knowns = |states: &IndexMap<Value, State>| {
        states
            .iter()
            .filter_map(|(value, state)| match state {
                State::Known(fact) => Some((*value, fact.clone())),
                _ => None,
            })
            .collect::<IndexMap<_, _>>()
    };
    loop {
        let Some(value) = pending.pop_front() else {
            let facts = knowns(&states);
            let mut changed = false;
            let mut deferred = Vec::new();
            if let Some(successors) = successors {
                for at in live.iter().copied().collect::<Vec<_>>() {
                    let Some(selected) = successors(blocks[&at], &facts, &states) else {
                        deferred.push(at);
                        continue;
                    };
                    for target in selected {
                        changed |= activate(at, target, &mut live, &mut edges, &mut pending, &mut queued);
                    }
                }
            }
            if !pending.is_empty() || changed {
                continue;
            }
            let unresolved = recipes
                .keys()
                .copied()
                .filter(|value| live.contains(&owners[value]) && states[value] == State::Pending)
                .collect::<Vec<_>>();
            for value in &unresolved {
                states.insert(*value, State::Overdefined);
                enqueue(consumers_of(value), &live, &mut pending, &mut queued);
            }
            if !unresolved.is_empty() {
                continue;
            }
            for at in deferred {
                for target in blocks[&at].succ.clone() {
                    changed |= activate(at, target, &mut live, &mut edges, &mut pending, &mut queued);
                }
            }
            if changed || !pending.is_empty() {
                continue;
            }
            return facts;
        };
        queued.remove(&value);
        if seeds.contains_key(&value) || states[&value] == State::Overdefined {
            continue;
        }
        let recipe = &recipes[&value];
        let mut candidate = State::Pending;
        match recipe {
            Recipe::Phi(phi) => {
                if successors.is_some() && owners[&value] == body.entry {
                    // entry also executes before any backedge
                    candidate = State::Overdefined;
                }
                for (predecessor, incoming) in phi.incoming.iter() {
                    if successors.is_some() && !edges.contains(&(*predecessor, owners[&value])) {
                        continue;
                    }
                    candidate = _meet(candidate, states.get(incoming).cloned().unwrap_or(State::Overdefined));
                }
            }
            Recipe::Op(op)
                if !supported(op.kind) || !op.loads.is_empty() || !op.stores.is_empty() || op.barrier() =>
            {
                candidate = State::Overdefined;
            }
            Recipe::Op(op) => {
                let facts = op
                    .args
                    .iter()
                    .filter_map(|arg| match arg {
                        Arg::Held(held) => match states.get(&held.value) {
                            Some(State::Known(fact)) => Some((held.value, fact.clone())),
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect::<IndexMap<_, _>>();
                candidate = match consts::_result(op, &facts, None, None) {
                    Some(fact) => State::Known(fact),
                    None => {
                        let inputs = op
                            .args
                            .iter()
                            .filter_map(|arg| match arg {
                                Arg::Held(held) => Some(states.get(&held.value).cloned().unwrap_or(State::Overdefined)),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        if inputs.contains(&State::Pending) && !inputs.contains(&State::Overdefined) {
                            State::Pending
                        } else {
                            State::Overdefined
                        }
                    }
                };
            }
        }
        let merged = _meet(states[&value].clone(), candidate);
        if merged == states[&value] {
            continue;
        }
        states.insert(value, merged);
        enqueue(consumers_of(&value), &live, &mut pending, &mut queued);
    }
}

#[cfg(test)]
#[path = "constant_cycles_tests.rs"]
mod tests;
