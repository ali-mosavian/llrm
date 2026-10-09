//! Sparse value propagation with distinct pending and overdefined states:
//! llrm-core's `analysis/constant_cycles.rs`, a port of
//! `qbopt/analysis/constant_cycles.py`, adapted to the rich MIR. An
//! optional successor evaluator discovers executable edges. Pending values
//! are never interpreted as LLVM undef: unresolved reachable values become
//! overdefined before the final result, and unresolved branches retain
//! every successor.
//!
//! A phi's incoming constant is known, as the old MIR's copy of one was.
//!
//! Skipped, `test_cyclic_propagation_does_not_widen_a_known_word`: a phi's
//! incoming values share its type, so a word cannot meet a long there.

use std::collections::{BTreeSet, VecDeque};

use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_support::hash::{IndexMap, IndexSet};
use llrm_support::pyset::PySet;

use crate::cfg;
use crate::consts::{self, Known};
use crate::memory::Unit;

/// Python's `State | Known`: a value's lattice cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum State {
    Pending,
    Overdefined,
    Known(Known),
}

/// The successor evaluator, given a block's id: `None` defers the block's
/// branch.
pub type Successors<'a> = &'a dyn Fn(i64, &IndexMap<ValueId, Known>, &IndexMap<ValueId, State>) -> Option<Vec<i64>>;

fn _meet(
    first: State,
    second: State,
) -> State {
    if first == State::Pending {
        return second;
    }
    if second == State::Pending || first == second {
        return first;
    }
    State::Overdefined
}

pub fn propagated(
    unit: &Unit,
    seeds: &IndexMap<ValueId, Known>,
    successors: Option<Successors<'_>>,
) -> IndexMap<ValueId, Known> {
    let function = unit.function;
    let Some(entry) = function.entry().map(cfg::id) else {
        return seeds.clone();
    };
    let is_phi = |inst: InstId| function.instruction(inst).opcode == Opcode::Phi;
    // Phis first, then the other operations, as the old MIR kept them apart.
    let mut recipes = IndexMap::<ValueId, InstId>::default();
    let mut owners = IndexMap::<ValueId, i64>::default();
    for phis in [true, false] {
        for (block, inst) in function.walk().filter(|(_, inst)| is_phi(*inst) == phis) {
            if let Some(value) = consts::_defined(unit, inst) {
                recipes.insert(value, inst);
                owners.insert(value, cfg::id(block));
            }
        }
    }
    let mut states = recipes
        .keys()
        .map(|value| (*value, seeds.get(value).cloned().map_or(State::Pending, State::Known)))
        .collect::<IndexMap<_, _>>();
    for (value, fact) in seeds {
        states.insert(*value, State::Known(fact.clone()));
    }
    let values = |inst: InstId| -> Vec<Operand> {
        let op = function.instruction(inst);
        if is_phi(inst) { op.operands.iter().step_by(2).copied().collect() } else { op.operands.clone() }
    };
    let mut consumers = IndexMap::<ValueId, IndexSet<ValueId>>::default();
    for (value, recipe) in &recipes {
        for incoming in values(*recipe) {
            if let Operand::Value(incoming) = incoming {
                consumers.entry(incoming).or_default().insert(*value);
            }
        }
    }
    let graph = cfg::graph(function);
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<IndexMap<_, _>>();
    let mut live = match successors {
        None => blocks.keys().copied().collect::<PySet<i64>>(),
        Some(_) => [entry].into_iter().collect(),
    };
    let mut edges = BTreeSet::<(i64, i64)>::new();
    let mut pending = recipes.keys().copied().filter(|value| live.contains(&owners[value])).collect::<VecDeque<_>>();
    let mut queued = pending.iter().copied().collect::<BTreeSet<_>>();

    let enqueue =
        |values: Vec<ValueId>, live: &PySet<i64>, pending: &mut VecDeque<ValueId>, queued: &mut BTreeSet<ValueId>| {
            for value in values {
                if !queued.contains(&value) && live.contains(&owners[&value]) {
                    pending.push_back(value);
                    queued.insert(value);
                }
            }
        };
    let consumers_of = |value: &ValueId| {
        consumers.get(value).map(|users| users.iter().copied().collect::<Vec<_>>()).unwrap_or_default()
    };

    let activate = |source: i64,
                    target: i64,
                    live: &mut PySet<i64>,
                    edges: &mut BTreeSet<(i64, i64)>,
                    pending: &mut VecDeque<ValueId>,
                    queued: &mut BTreeSet<ValueId>| {
        if !blocks.contains_key(&target) || edges.contains(&(source, target)) {
            return false;
        }
        edges.insert((source, target));
        if !live.contains(&target) {
            live.add(target);
            let values = recipes.keys().copied().filter(|value| owners[value] == target).collect();
            enqueue(values, live, pending, queued);
        } else {
            let values = recipes
                .iter()
                .filter(|(value, inst)| owners[*value] == target && is_phi(**inst))
                .map(|(value, _)| *value)
                .collect();
            enqueue(values, live, pending, queued);
        }
        true
    };
    let supported = |opcode: &Opcode| match opcode {
        Opcode::Binary(kind) => consts::folds(*kind),
        Opcode::Cast(CastOp::Trunc | CastOp::SExt) => true,
        _ => false,
    };
    let knowns = |states: &IndexMap<ValueId, State>| {
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
                    let Some(selected) = successors(at, &facts, &states) else {
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
        let recipe = recipes[&value];
        let op = function.instruction(recipe);
        let state = |one: Operand| match one {
            Operand::Value(one) => states.get(&one).cloned().unwrap_or(State::Overdefined),
            constant => {
                consts::_operand(unit, constant, &IndexMap::default(), None).map_or(State::Overdefined, State::Known)
            }
        };
        let mut candidate = State::Pending;
        if is_phi(recipe) {
            if successors.is_some() && owners[&value] == entry {
                // entry also executes before any backedge
                candidate = State::Overdefined;
            }
            for pair in op.operands.chunks(2) {
                let [incoming, Operand::Block(predecessor)] = pair else { continue };
                if successors.is_some() && !edges.contains(&(cfg::id(*predecessor), owners[&value])) {
                    continue;
                }
                candidate = _meet(candidate, state(*incoming));
            }
        } else if !supported(&op.opcode) {
            candidate = State::Overdefined;
        } else {
            let facts = op
                .operands
                .iter()
                .filter_map(|&one| match (one, state(one)) {
                    (Operand::Value(one), State::Known(fact)) => Some((one, fact)),
                    _ => None,
                })
                .collect::<IndexMap<_, _>>();
            candidate = match consts::_result(unit, recipe, &facts, None) {
                Some(fact) => State::Known(fact),
                None => {
                    let inputs = op
                        .operands
                        .iter()
                        .filter(|one| matches!(one, Operand::Value(_)))
                        .map(|&one| state(one))
                        .collect::<Vec<_>>();
                    if inputs.contains(&State::Pending) && !inputs.contains(&State::Overdefined) {
                        State::Pending
                    } else {
                        State::Overdefined
                    }
                }
            };
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
