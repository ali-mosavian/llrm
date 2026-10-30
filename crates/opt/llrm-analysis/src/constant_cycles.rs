//! Sparse value propagation over phis, with distinct pending and overdefined
//! states, to find constants that only a cycle proves.
//!
//! # How it works
//! Each value has a state: `Pending` (nothing met yet), `Known(fact)` or
//! `Overdefined`, with `Pending` the identity of the meet and two different
//! facts meeting to `Overdefined`. `propagated(unit, seeds, successors, only)`
//! takes the facts already known (`seeds`, from [`crate::consts`]) and settles
//! the rest, which are the phis no seed has and the operations that depend on
//! them: all of them, or the values `only` names (an incremental `Registers`
//! brings up to date the values a change reached and no others). Values that
//! cannot come out known (one made from an argument, a load or a call) are left
//! out of the propagation. A body with no such open phi returns its seeds at
//! once.
//!
//! A worklist re-evaluates a value when one of its inputs changes. A phi meets
//! the facts of its incoming values (constants evaluate through
//! `consts::_operand`); a supported operation (a foldable binary operation,
//! `trunc`, `sext`) folds through `consts::_result` when every input is known,
//! stays pending while some input is pending and none is overdefined, and
//! otherwise is overdefined. When nothing is left on the worklist and pending
//! values remain, they are made overdefined and propagation continues.
//! Pending is never read as LLVM `undef`: an unresolved value is overdefined
//! before the result is returned.
//!
//! ```text
//!  b1:  %i = phi [0, b0], [%j, b1]    %i: meet(Known 0, pending) = Known 0
//!       %j = add %i, 0                %j: Known 0
//!                                     %i again: meet(Known 0, Known 0), stable
//! ```
//!
//! An optional successor evaluator (`Successors { choose, reads }`) finds the
//! executable edges. Only the entry is live at first; a block's `choose`
//! answers which successors run from the facts so far, and is asked again only
//! when a value in `reads` for that block changes. A phi then meets only the
//! incoming values of executable edges, and the entry's phis also meet
//! `Overdefined`, since the entry runs before any back edge. A block whose
//! branch `choose` defers (`None`) keeps every successor once nothing else is
//! left to resolve.
//!
//! # Example
//! A loop whose carried value adds 0 to itself, so only the cycle proves it:
//!
//! ```mir
//! define void @f(i32 %incoming, i1 %more) {
//! b0:
//!   br label %b10
//!
//! b10:
//!   %joined = phi i32 [ 7, %b0 ], [ %carried, %b10 ]
//!   %carried = add i32 %joined, 0
//!   br i1 %more, label %b10, label %b20
//!
//! b20:
//!   ret void
//! }
//! ```
//!
//! | Value | Fact |
//! | --- | --- |
//! | `%joined` | `Known { n: 7, width: 32 }` |
//! | `%carried` | `Known { n: 7, width: 32 }` |
//!
//! With step 1, or with `%incoming` as the source of the add, neither is known.
//!
//! From unit test `test_unchanged_loop_value_is_constant_through_its_backedge`
//! in `constant_cycles_tests.rs`.
//!
//! # Provides
//! `State`, `Successors` and `propagated`. Consumers: [`crate::consts`]
//! (`known`, with no evaluator) and `decide` in `llrm-transforms`, which
//! passes an evaluator to find which branches fold.
//!
//! # Needs
//! [`crate::consts`] for the per-operation folding, and [`crate::cfg`].
//!
//! # See also
//! LLVM SCCP (`SCCPSolver`) and its `unknown`, `constant`, `overdefined`
//! lattice. `LLRM_CHECK_CYCLES` re-derives the result without the early
//! return and asserts it matches.

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

/// What a value is folded from: the operations `propagated` evaluates.
fn _supported(opcode: &Opcode) -> bool {
    match opcode {
        Opcode::Binary(kind) => consts::folds(*kind),
        Opcode::Cast(CastOp::Trunc | CastOp::SExt) => true,
        _ => false,
    }
}

/// The values that can come out known: a phi, or an operation `_supported`
/// evaluates, all of whose operands can. Any other value is overdefined at the
/// end whatever the rest is (an operand that is overdefined makes a phi and a
/// fold overdefined; `x ^ x` and `x - x` are zero for any `x`), so the
/// propagation does not set a recipe up for it: a body of N counted loops has N
/// phis to settle and every other value of it, which was 70% of what `analysis
/// registers` cost at N=64.
fn _possible(
    unit: &Unit,
    relevant: &[(llrm_mir::module::BlockId, InstId)],
    seeded: &dyn Fn(&ValueId) -> bool,
) -> llrm_mir::dense::IdSet<ValueId> {
    use llrm_mir::dense::{IdMap, IdSet};
    let function = unit.function;
    let zeroes = |op: &llrm_mir::module::Instruction| {
        matches!(
            op.opcode,
            Opcode::Binary(llrm_mir::opcode::BinaryOp::Xor | llrm_mir::opcode::BinaryOp::Sub)
        )
            && matches!(op.operands[0], Operand::Value(_))
            && op.operands[0] == op.operands[1]
    };
    let mut possible = IdSet::<ValueId>::new();
    fn named<'a>(
        op: &'a llrm_mir::module::Instruction,
        zero: bool,
    ) -> impl Iterator<Item = ValueId> + 'a {
        let step = if op.opcode == Opcode::Phi { 2 } else { 1 };
        let kept = if zero { 0 } else { op.operands.len() };
        op.operands
            .iter()
            .take(kept)
            .step_by(step)
            .filter_map(
                |one| match one {
                    Operand::Value(one) => Some(*one),
                    _ => None,
                },
            )
    }
    let mut candidates = Vec::<(ValueId, InstId)>::new();
    for &(_, inst) in relevant {
        let op = function.instruction(inst);
        let Some(value) = consts::_defined(unit, inst).filter(|value| !seeded(value)) else { continue };
        if op.opcode == Opcode::Phi || _supported(&op.opcode) || zeroes(op) {
            possible.insert(value);
            candidates.push((value, inst));
        }
    }
    let mut users = IdMap::<ValueId, Vec<ValueId>>::new();
    let mut gone = Vec::<ValueId>::new();
    for &(value, inst) in &candidates {
        for one in named(function.instruction(inst), zeroes(function.instruction(inst))) {
            if !seeded(&one) && !possible.contains(&one) {
                if possible.remove(&value) {
                    gone.push(value);
                }
            } else {
                users.get_or_insert_with(one, Vec::new).push(value);
            }
        }
    }
    while let Some(value) = gone.pop() {
        for &user in users.get(&value).into_iter().flatten() {
            if possible.remove(&user) {
                gone.push(user);
            }
        }
    }
    possible
}

thread_local! {
    /// 1 while `LLRM_CHECK_CYCLES` works a propagation twice, 2 for the run
    /// that leaves no value out.
    static CHECKING: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// `only`, where given, names the values to settle: the others are final, as
/// `seeds` has them or not at all.
pub fn propagated(
    unit: &Unit,
    seeds: IndexMap<ValueId, Known>,
    successors: Option<Successors<'_>>,
    only: Option<&llrm_mir::dense::IdSet<ValueId>>,
) -> IndexMap<ValueId, Known> {
    if successors.is_none() && CHECKING.get() == 0 && llrm_support::env_flag!("LLRM_CHECK_CYCLES") {
        CHECKING.set(1);
        let found = propagated(unit, seeds.clone(), successors, only);
        CHECKING.set(2);
        let whole = propagated(unit, seeds, successors, only);
        CHECKING.set(0);
        assert!(found == whole, "constant cycles: leaving out the values that cannot be known changed the facts");
        return found;
    }
    let function = unit.function;
    let Some(entry) = function.entry().map(cfg::id) else {
        return seeds;
    };
    let is_phi = |inst: InstId| function.instruction(inst).opcode == Opcode::Phi;
    // The instructions that make the values to settle, in layout order where
    // all are.
    let relevant: Vec<(llrm_mir::module::BlockId, InstId)> = match only {
        None => function.walk().collect(),
        Some(only) => only
            .iter()
            .filter_map(|value| match function.value(value).def {
                llrm_mir::module::ValueDef::Instruction(inst) => function.parent(inst).map(|block| (block, inst)),
                llrm_mir::module::ValueDef::Argument(_) => None,
            })
            .collect(),
    };
    // A body with no phi that lacks a seed has nothing to settle: no more is
    // set up for it.
    let unseeded = |(_, inst): &(_, InstId)| {
        is_phi(*inst) && consts::_defined(unit, *inst).is_some_and(|value| !seeds.contains_key(&value))
    };
    if !relevant.iter().any(unseeded) && !llrm_support::env_flag!("LLRM_CHECK_CYCLES") {
        return seeds;
    }
    let possible = (successors.is_none() && CHECKING.get() != 2)
        .then(|| _possible(unit, &relevant, &|value| seeds.contains_key(value)));
    let possible = possible.as_ref();
    // What is left to propagate is a phi no seed has: every other value is as
    // `seeds` has it, or derives from them. Setting the propagation up (a
    // recipe for every value, the consumers, a copy of the graph) was 1.8% of
    // the -O1 compile of QCport, 25000 times for bodies with nothing to
    // resolve. LLRM_CHECK_CYCLES works it out anyway and asserts it comes to
    // the seeds.
    let open = relevant
        .iter()
        .filter(|(_, inst)| is_phi(*inst))
        .any(
            |&(_, inst)| consts::_defined(unit, inst)
                .is_some_and(|value| !seeds.contains_key(&value) && possible.is_none_or(|kept| kept.contains(&value))),
        );
    if !open && !llrm_support::env_flag!("LLRM_CHECK_CYCLES") {
        return seeds;
    }
    #[cfg(test)]
    WORKED.with(|worked| worked.set(worked.get() + 1));
    // Phis first, then the other operations, as the old MIR kept them apart.
    let mut recipes = IndexMap::<ValueId, InstId>::default();
    let mut owners = IndexMap::<ValueId, i64>::default();
    for phis in [true, false] {
        for &(block, inst) in relevant.iter().filter(|(_, inst)| is_phi(*inst) == phis) {
            if let Some(value) = consts::_defined(unit, inst)
                .filter(|value| possible.is_none_or(|kept| kept.contains(value) || seeds.contains_key(value)))
            {
                recipes.insert(value, inst);
                owners.insert(value, cfg::id(block));
            }
        }
    }
    let mut states = recipes
        .keys()
        .map(|value| (*value, seeds.get(value).cloned().map_or(State::Pending, State::Known)))
        .collect::<IndexMap<_, _>>();
    // A body settled by `only` or by the values it has no edges for reads the
    // seeds as they are.
    let sparse = successors.is_none();
    if !sparse {
        for (value, fact) in &seeds {
            states.insert(*value, State::Known(fact.clone()));
        }
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
    let graph = if successors.is_some() { cfg::graph(function) } else { Vec::new() };
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<IndexMap<_, _>>();
    let mut live = match successors {
        None => owners.values().copied().collect::<PySet<i64>>(),
        Some(_) => [entry].into_iter().collect(),
    };
    let mut edges = BTreeSet::<(i64, i64)>::new();
    let mut pending = recipes.keys().copied().filter(|value| live.contains(&owners[value])).collect::<VecDeque<_>>();
    let mut queued = pending.iter().copied().collect::<llrm_mir::dense::IdSet<ValueId>>();

    let enqueue = |values: Vec<ValueId>,
                   live: &PySet<i64>,
                   pending: &mut VecDeque<ValueId>,
                   queued: &mut llrm_mir::dense::IdSet<ValueId>| {
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
                    queued: &mut llrm_mir::dense::IdSet<ValueId>,
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
    let supported = _supported;
    let knowns = |states: &IndexMap<ValueId, State>| {
        let mut all = IndexMap::default();
        all.extend(states.iter().filter_map(|(value, state)| match state {
            State::Known(fact) => Some((*value, fact.clone())),
            _ => None,
        }));
        all
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
            // The seeds are no longer the fixed point of the body's constants
            // (the manager keeps none, `Registers::run`): a body
            // with no open phi may come to more than its seeds, and
            // may not come to less or to other.
            assert!(
                open || seeds.iter().all(|(value, fact)| facts.get(value) == Some(fact)),
                "constant cycles: a body with no open phi contradicted its seeds"
            );
            // The evaluator is asked only where a value it reads changed: asked
            // of every live block now, it names no edge that is not
            // already there.
            if llrm_support::env_flag!("LLRM_CHECK_CYCLES") {
                assert!(
                    recipes.keys().all(|value| !live.contains(&owners[value]) || states[value] != State::Pending),
                    "constant cycles: a live value was left with no state"
                );
            }
            if let (Some(successors), true) = (successors, llrm_support::env_flag!("LLRM_CHECK_CYCLES")) {
                for at in live.iter().copied() {
                    let reached = match (successors.choose)(at, &facts, &states) {
                        Some(selected) => selected,
                        None => blocks[&at].succ.to_vec(),
                    };
                    assert!(
                        reached.iter().all(|target| !blocks.contains_key(target) || edges.contains(&(at, *target))),
                        "constant cycles: block {at} names an edge the worklist did not reach"
                    );
                }
            }
            return if sparse {
                let mut all = seeds;
                all.extend(facts);
                all
            } else {
                facts
            };
        };
        queued.remove(&value);
        if seeds.contains_key(&value) || states[&value] == State::Overdefined {
            continue;
        }
        let recipe = recipes[&value];
        let op = function.instruction(recipe);
        let state = |one: Operand| match one {
            Operand::Value(one) => match states.get(&one) {
                Some(found) => found.clone(),
                None => seeds.get(&one).cloned().map_or(State::Overdefined, State::Known),
            },
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
