//! Sparse value propagation through cyclic MIR definitions.
//!
//! Direct port of `qbopt.analysis.constant_cycles:State`, `_meet`, and the
//! `successors=None` path of `propagated`.  The default constant analysis has
//! no executable-edge question: every block participates, and a phi is a
//! number only when every incoming value reaches the identical fact.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::consts::{self, Known};
use crate::model::mir::{Arg, Kind, MirBody, Op, Phi, Value};

/// A definition not yet solved, or one which cannot be a constant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum State {
    Pending,
    Overdefined,
    Known(Known),
}

/// Python's `_meet(first, second)`.
fn _meet(first: State, second: State) -> State {
    if first == State::Pending {
        second
    } else if second == State::Pending || first == second {
        first
    } else {
        State::Overdefined
    }
}

enum Recipe<'a> {
    Phi(&'a Phi),
    Op(&'a Op),
}

fn known(states: &BTreeMap<Value, State>) -> BTreeMap<Value, Known> {
    states
        .iter()
        .filter_map(|(value, state)| match state {
            State::Known(fact) => Some((*value, fact.clone())),
            State::Pending | State::Overdefined => None,
        })
        .collect()
}

/// Python's `propagated(body, seeds, successors=None)`.
pub(super) fn propagated(body: &MirBody, seeds: BTreeMap<Value, Known>) -> BTreeMap<Value, Known> {
    // Python's dict comprehensions retain the last definition when an
    // hand-built body repeats a value.  Keep that order-dependent source
    // contract rather than treating duplicate definitions as a new error.
    let mut recipes = BTreeMap::new();
    let mut owners = BTreeMap::new();
    for block in &body.blocks {
        for phi in &block.phis {
            recipes.insert(phi.result, Recipe::Phi(phi));
            owners.insert(phi.result, block.at);
        }
    }
    for block in &body.blocks {
        for op in &block.ops {
            if let Some(value) = consts::_defined(op) {
                recipes.insert(value, Recipe::Op(op));
                owners.insert(value, block.at);
            }
        }
    }

    let mut states = recipes
        .keys()
        .map(|value| {
            (
                *value,
                seeds
                    .get(value)
                    .cloned()
                    .map(State::Known)
                    .unwrap_or(State::Pending),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (value, fact) in &seeds {
        states.insert(*value, State::Known(fact.clone()));
    }

    let mut consumers = BTreeMap::<Value, BTreeSet<Value>>::new();
    for (value, recipe) in &recipes {
        let inputs: Vec<Value> = match recipe {
            Recipe::Phi(phi) => phi.incoming.values().copied().collect(),
            Recipe::Op(op) => op
                .args
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Held(held) => Some(held.value),
                    Arg::Const(_)
                    | Arg::Symbol(_)
                    | Arg::FrameAddress(_)
                    | Arg::FrameSelector(_)
                    | Arg::Cell(_)
                    | Arg::Opaque(_) => None,
                })
                .collect(),
        };
        for input in inputs {
            consumers.entry(input).or_default().insert(*value);
        }
    }

    let live = body
        .blocks
        .iter()
        .map(|block| block.at)
        .collect::<BTreeSet<_>>();
    let mut pending = recipes
        .keys()
        .copied()
        .filter(|value| live.contains(&owners[value]))
        .collect::<VecDeque<_>>();
    let mut queued = pending.iter().copied().collect::<BTreeSet<_>>();
    loop {
        let Some(value) = pending.pop_front() else {
            let unresolved = recipes
                .keys()
                .copied()
                .filter(|value| {
                    live.contains(&owners[value]) && states.get(value) == Some(&State::Pending)
                })
                .collect::<Vec<_>>();
            if unresolved.is_empty() {
                return known(&states);
            }
            for value in unresolved {
                states.insert(value, State::Overdefined);
                if let Some(users) = consumers.get(&value) {
                    for user in users {
                        if queued.insert(*user) {
                            pending.push_back(*user);
                        }
                    }
                }
            }
            continue;
        };
        queued.remove(&value);
        if seeds.contains_key(&value) || states.get(&value) == Some(&State::Overdefined) {
            continue;
        }
        let recipe = &recipes[&value];
        let candidate = match recipe {
            Recipe::Phi(phi) => phi
                .incoming
                .values()
                .fold(State::Pending, |state, incoming| {
                    _meet(
                        state,
                        states.get(incoming).cloned().unwrap_or(State::Overdefined),
                    )
                }),
            Recipe::Op(op)
                if !matches!(
                    op.kind,
                    Kind::Add
                        | Kind::Sub
                        | Kind::And
                        | Kind::Or
                        | Kind::Xor
                        | Kind::Shl
                        | Kind::Shr
                        | Kind::Mul
                        | Kind::Neg
                        | Kind::Not
                        | Kind::Copy
                        | Kind::Extract
                        | Kind::SignExtend
                        | Kind::Concat
                        | Kind::Smulhi
                ) || !op.loads.is_empty()
                    || !op.stores.is_empty()
                    || op.barrier() =>
            {
                State::Overdefined
            }
            Recipe::Op(op) => {
                let facts = known(&states);
                if let Some(fact) = consts::_result(op, &facts, None) {
                    State::Known(fact)
                } else {
                    let inputs = op
                        .args
                        .iter()
                        .filter_map(|arg| match arg {
                            Arg::Held(held) => Some(
                                states
                                    .get(&held.value)
                                    .cloned()
                                    .unwrap_or(State::Overdefined),
                            ),
                            Arg::Const(_)
                            | Arg::Symbol(_)
                            | Arg::FrameAddress(_)
                            | Arg::FrameSelector(_)
                            | Arg::Cell(_)
                            | Arg::Opaque(_) => None,
                        })
                        .collect::<Vec<_>>();
                    if inputs.contains(&State::Pending) && !inputs.contains(&State::Overdefined) {
                        State::Pending
                    } else {
                        State::Overdefined
                    }
                }
            }
        };
        let state = states.get(&value).cloned().unwrap_or(State::Overdefined);
        let merged = _meet(state.clone(), candidate);
        if merged == state {
            continue;
        }
        states.insert(value, merged);
        if let Some(users) = consumers.get(&value) {
            for user in users {
                if queued.insert(*user) {
                    pending.push_back(*user);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::consts::{Known, known};
    use crate::model::mir::{Arg, Const, Held, Kind, MirBlock, MirBody, Op, Phi, Value};

    fn value(id: u32, at: i64) -> Value {
        Value::new(id, at)
    }

    /// Direct port of `tests/test_constant_cycles.py:body_with_cycle`.
    fn body_with_cycle(step: i64, external: bool) -> (MirBody, Value, Value) {
        let (start, joined, carried, incoming) =
            (value(1, 0), value(2, 10), value(3, 10), value(4, 10));
        let mut seed = Op::new(0, None, "", vec![start], vec![]);
        seed.kind = Kind::Copy;
        seed.args = vec![Arg::Const(Const::new(7, 4))];
        seed.results = vec![Arg::Held(Held {
            value: start,
            width: 4,
        })];
        let source = if external { incoming } else { joined };
        let mut update = Op::new(10, None, "", vec![carried], vec![source]);
        update.kind = Kind::Add;
        update.args = vec![
            Arg::Held(Held {
                value: source,
                width: 4,
            }),
            Arg::Const(Const::new(step, 4)),
        ];
        update.results = vec![Arg::Held(Held {
            value: carried,
            width: 4,
        })];
        let mut incoming_values = crate::model::mir::OrderedMap::new();
        incoming_values.insert(0, start);
        incoming_values.insert(10, carried);
        (
            MirBody::new(
                0,
                vec![
                    MirBlock::new(0, vec![], vec![seed], vec![10]),
                    MirBlock::new(
                        10,
                        vec![Phi {
                            result: joined,
                            incoming: incoming_values,
                        }],
                        vec![update],
                        vec![10, 20],
                    ),
                    MirBlock::new(20, vec![], vec![], vec![]),
                ],
            ),
            joined,
            carried,
        )
    }

    #[test]
    fn direct_constant_cycles_unchanged_loop_value_is_constant_through_backedge() {
        let (body, joined, carried) = body_with_cycle(0, false);
        let facts = known(&body);
        assert_eq!(facts.get(&joined), Some(&Known::new(7, 4)));
        assert_eq!(facts.get(&carried), Some(&Known::new(7, 4)));
    }

    #[test]
    fn direct_constant_cycles_changed_runtime_and_unanchored_cycles_remain_unknown() {
        for (step, external) in [(1, false), (0, true)] {
            let (body, joined, carried) = body_with_cycle(step, external);
            let facts = known(&body);
            assert!(!facts.contains_key(&joined));
            assert!(!facts.contains_key(&carried));
        }
        let (mut body, joined, carried) = body_with_cycle(0, false);
        body.blocks[1].phis[0].incoming.remove(&0);
        let facts = known(&body);
        assert!(!facts.contains_key(&joined));
        assert!(!facts.contains_key(&carried));
    }

    #[test]
    fn direct_constant_cycles_do_not_widen_a_known_word() {
        // Direct port of
        // `tests/test_constant_cycles.py:test_cyclic_propagation_does_not_widen_a_known_word`.
        let (mut body, joined, carried) = body_with_cycle(0, false);
        let seed = &mut body.blocks[0].ops[0];
        seed.args = vec![Arg::Const(Const::new(7, 2))];
        seed.results = vec![Arg::Held(Held {
            value: seed.defines[0],
            width: 2,
        })];
        let facts = known(&body);
        assert!(!facts.contains_key(&joined));
        assert!(!facts.contains_key(&carried));
    }

    #[test]
    fn direct_constant_cycles_are_independent_of_block_order() {
        let (body, _, _) = body_with_cycle(0, false);
        let mut reordered = body.clone();
        reordered.blocks.reverse();
        assert_eq!(known(&body), known(&reordered));
    }
}
