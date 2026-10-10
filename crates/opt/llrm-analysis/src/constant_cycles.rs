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
pub type Chooser<'a> = &'a dyn Fn(i64, &IndexMap<ValueId, Known>, &IndexMap<ValueId, State>) -> Option<Vec<i64>>;

/// How the executable edges are found: the evaluator, and the values it reads
/// of a block (its answer is the same until one of them changes, so it is asked
/// again then and not at every round).
#[derive(Clone, Copy)]
pub struct Successors<'a> {
    pub choose: Chooser<'a>,
    pub reads: &'a dyn Fn(i64) -> Vec<ValueId>,
}

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

#[cfg(test)]
thread_local! {
    /// Propagations set up, for a test that a body with no open phi does not
    /// set one up.
    pub static WORKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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
    // What is left to propagate is a phi no seed has: every other value is as
    // `seeds` has it, or derives from them. Setting the propagation up (a
    // recipe for every value, the consumers, a copy of the graph) was 1.8% of
    // the -O1 compile of QCport, 25000 times for bodies with nothing to
    // resolve. LLRM_CHECK_CYCLES works it out anyway and asserts it comes to
    // the seeds.
    let open = function
        .walk()
        .filter(|(_, inst)| is_phi(*inst))
        .any(|(_, inst)| consts::_defined(unit, inst).is_some_and(|value| !seeds.contains_key(&value)));
    if !open && !llrm_support::env_set("LLRM_CHECK_CYCLES") {
        return seeds.clone();
    }
    #[cfg(test)]
    WORKED.with(|worked| worked.set(worked.get() + 1));
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

    // The values each block makes, in the order of `recipes`, and its phis.
    let incremental = successors.is_some();
    let mut made = IndexMap::<i64, Vec<ValueId>>::default();
    if incremental {
        for (value, owner) in &owners {
            made.entry(*owner).or_default().push(*value);
        }
    }
    let activate = |source: i64,
                    target: i64,
                    live: &mut PySet<i64>,
                    edges: &mut BTreeSet<(i64, i64)>,
                    pending: &mut VecDeque<ValueId>,
                    queued: &mut BTreeSet<ValueId>,
                    dirty: &mut BTreeSet<i64>,
                    fresh: &mut Vec<i64>| {
        if !blocks.contains_key(&target) || edges.contains(&(source, target)) {
            return false;
        }
        edges.insert((source, target));
        let here = made.get(&target).map(Vec::as_slice).unwrap_or(&[]);
        if !live.contains(&target) {
            live.add(target);
            dirty.insert(target);
            fresh.push(target);
            enqueue(here.to_vec(), live, pending, queued);
        } else {
            let phis = here.iter().copied().filter(|value| is_phi(recipes[value])).collect();
            enqueue(phis, live, pending, queued);
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
    // The blocks whose successors are to be asked again: those just live and
    // those that read a value whose state changed. The answers of the
    // others are as they were.
    let mut dirty: BTreeSet<i64> = if incremental { live.iter().copied().collect() } else { BTreeSet::new() };
    // The live values with no state yet, by place in `recipes`: what is left to
    // settle when the propagation stalls, kept as the states change.
    let mut waiting: BTreeSet<usize> = if incremental {
        recipes
            .iter()
            .enumerate()
            .filter(|(_, (value, _))| live.contains(&owners[*value]) && states[*value] == State::Pending)
            .map(|(place, _)| place)
            .collect()
    } else {
        BTreeSet::new()
    };
    let mut readers = llrm_mir::dense::IdMap::<ValueId, Vec<i64>>::new();
    if let Some(successors) = successors {
        for block in &graph {
            for value in (successors.reads)(block.at) {
                readers.get_or_insert_with(value, Vec::new).push(block.at);
            }
        }
    }
    // What the evaluator saw last as facts, kept as the states change.
    let mut fresh = Vec::<i64>::new();
    let mut known_now = if incremental { knowns(&states) } else { IndexMap::default() };
    let mut deferred_now = BTreeSet::<i64>::new();
    loop {
        let Some(value) = pending.pop_front() else {
            let mut changed = false;
            let mut deferred = Vec::new();
            if let Some(successors) = successors {
                for at in std::mem::take(&mut dirty) {
                    if !live.contains(&at) {
                        continue;
                    }
                    let Some(selected) = (successors.choose)(at, &known_now, &states) else {
                        deferred_now.insert(at);
                        continue;
                    };
                    deferred_now.remove(&at);
                    for target in selected {
                        changed |= activate(
                            at,
                            target,
                            &mut live,
                            &mut edges,
                            &mut pending,
                            &mut queued,
                            &mut dirty,
                            &mut fresh,
                        );
                    }
                }
                deferred.extend(deferred_now.iter().copied());
                for block in fresh.drain(..) {
                    waiting.extend(
                        made.get(&block)
                            .into_iter()
                            .flatten()
                            .filter(|value| states[*value] == State::Pending)
                            .filter_map(|value| recipes.get_index_of(value)),
                    );
                }
            }
            if !pending.is_empty() || changed {
                continue;
            }
            let facts = knowns(&states);
            let unresolved = if incremental {
                std::mem::take(&mut waiting)
                    .into_iter()
                    .filter_map(|place| recipes.get_index(place).map(|(value, _)| *value))
                    .collect::<Vec<_>>()
            } else {
                recipes
                    .keys()
                    .copied()
                    .filter(|value| live.contains(&owners[value]) && states[value] == State::Pending)
                    .collect::<Vec<_>>()
            };
            for value in &unresolved {
                states.insert(*value, State::Overdefined);
                for &reader in readers.get(value).into_iter().flatten() {
                    dirty.insert(reader);
                }
                enqueue(consumers_of(value), &live, &mut pending, &mut queued);
            }
            if !unresolved.is_empty() {
                continue;
            }
            for at in deferred {
                for target in blocks[&at].succ.clone() {
                    changed |=
                        activate(at, target, &mut live, &mut edges, &mut pending, &mut queued, &mut dirty, &mut fresh);
                }
            }
            for block in fresh.drain(..) {
                waiting.extend(
                    made.get(&block)
                        .into_iter()
                        .flatten()
                        .filter(|value| states[*value] == State::Pending)
                        .filter_map(|value| recipes.get_index_of(value)),
                );
            }
            if changed || !pending.is_empty() {
                continue;
            }
            assert!(open || facts == *seeds, "constant cycles: a body with no open phi came to more than its seeds");
            // The evaluator is asked only where a value it reads changed: asked
            // of every live block now, it names no edge that is not
            // already there.
            if llrm_support::env_set("LLRM_CHECK_CYCLES") {
                assert!(
                    recipes.keys().all(|value| !live.contains(&owners[value]) || states[value] != State::Pending),
                    "constant cycles: a live value was left with no state"
                );
            }
            if let (Some(successors), true) = (successors, llrm_support::env_set("LLRM_CHECK_CYCLES")) {
                for at in live.iter().copied() {
                    let reached = match (successors.choose)(at, &facts, &states) {
                        Some(selected) => selected,
                        None => blocks[&at].succ.clone(),
                    };
                    assert!(
                        reached.iter().all(|target| !blocks.contains_key(target) || edges.contains(&(at, *target))),
                        "constant cycles: block {at} names an edge the worklist did not reach"
                    );
                }
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
        if incremental {
            match &merged {
                State::Known(fact) => {
                    known_now.insert(value, fact.clone());
                }
                _ => {
                    known_now.swap_remove(&value);
                }
            }
        }
        if incremental && merged != State::Pending {
            if let Some(place) = recipes.get_index_of(&value) {
                waiting.remove(&place);
            }
        }
        states.insert(value, merged);
        for &reader in readers.get(&value).into_iter().flatten() {
            dirty.insert(reader);
        }
        enqueue(consumers_of(&value), &live, &mut pending, &mut queued);
    }
}

#[cfg(test)]
#[path = "constant_cycles_tests.rs"]
mod tests;
