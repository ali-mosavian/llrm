//! Control-only facts for Python MIR induction analysis.
//!
//! Direct port of `qbopt.analysis.induction` loop-shape and affine-recurrence
//! facts: `LoopShape`, `Affine`, `canonical`, `invariant`, `test_only`,
//! `basics`, `_copied`, and `_stepped`.  This module deliberately does not
//! invent a portable-IR loop abstraction: Python MIR is the current stage
//! contract.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::mir::{Arg, Const, Held, Kind, MirBody, Op, OrderedMap};
use crate::model::mir_loops::{Loop, predecessors};

/// The canonical pre-tested, single-latch loop CFG.
///
/// Direct port of `qbopt.analysis.induction:LoopShape`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LoopShape {
    pub preheader: i64,
    pub latch: i64,
    pub entered: i64,
    pub exit: i64,
}

/// Python's `mir.Held | mir.Const` affine operand union.
///
/// Direct port of the closed annotation on `induction.Affine.start` and
/// `induction.Affine.step`; no other MIR operand can be a recurrence term.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum AffineOperand {
    Held(Held),
    Const(Const),
}

impl AffineOperand {
    const fn width(&self) -> u32 {
        match self {
            Self::Held(held) => held.width,
            Self::Const(constant) => constant.width,
        }
    }
}

/// `start + step * iteration`, in the loop this was asked about.
///
/// Direct port of `qbopt.analysis.induction:Affine`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Affine {
    pub value: u32,
    pub start: AffineOperand,
    pub step: AffineOperand,
    pub header: i64,
}

/// Python's `canonical(body, loop)`.
///
/// This keeps every structural refusal in the Python order.  In particular,
/// Python's final `blocks[at]` lookup is deliberately not softened: an
/// unknown non-header loop-body address is an invalid hand-built shape and
/// raises rather than becoming an ordinary structural refusal.
pub(crate) fn canonical(body: &MirBody, loop_: &Loop) -> Option<LoopShape> {
    // Python's dict comprehension retains the last duplicate address.
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    if loop_.latches.len() != 1 || !blocks.contains_key(&loop_.header) {
        return None;
    }
    let latch_at = *loop_.latches.first()?;
    let latch = blocks.get(&latch_at);
    let header = blocks.get(&loop_.header)?;
    let inside = &loop_.body;
    let predecessors = predecessors(&body.blocks);
    let outside = predecessors
        .get(&header.at)
        .into_iter()
        .flatten()
        .copied()
        .filter(|at| !inside.contains(at))
        .collect::<Vec<_>>();
    let entered = header
        .succ
        .iter()
        .copied()
        .filter(|at| inside.contains(at) && *at != header.at)
        .collect::<Vec<_>>();
    let exits = header
        .succ
        .iter()
        .copied()
        .filter(|at| !inside.contains(at))
        .collect::<Vec<_>>();

    let latch = latch?;
    if outside.len() != 1
        || blocks.get(&outside[0])?.succ.as_slice() != [header.at]
        || latch.succ.as_slice() != [header.at]
        || entered.len() != 1
        || exits.len() != 1
        || header.ops.is_empty()
        || header.ops.last()?.kind != Kind::Branch
        || inside
            .iter()
            .filter(|at| **at != header.at)
            .any(|at| blocks[at].succ.iter().any(|to| !inside.contains(to)))
    {
        return None;
    }
    Some(LoopShape {
        preheader: outside[0],
        latch: latch_at,
        entered: entered[0],
        exit: exits[0],
    })
}

/// Python's `test_only(op)`.
pub(crate) fn test_only(op: &Op) -> bool {
    if op.kind == Kind::Nothing
        && op.name.is_empty()
        && op.defines.is_empty()
        && op.uses.is_empty()
        && op.args.is_empty()
        && op.results.is_empty()
        && op.loads.is_empty()
        && op.stores.is_empty()
        && op.merges.is_empty()
        && !op.barrier()
        && op.floating.is_none()
        && op.stack.is_none()
        && op.floating_origin.is_none()
    {
        return true;
    }
    matches!(op.kind, Kind::Sub | Kind::And | Kind::Or)
        && op.results.is_empty()
        && op.loads.is_empty()
        && op.stores.is_empty()
        && op.merges.is_empty()
        && !op.barrier()
        && op.floating.is_none()
        && op.stack.is_none()
        && op.floating_origin.is_none()
        && !op.defines.is_empty()
        && op.defines.iter().all(|value| value.flags)
}

/// Python's `invariant(body, inside)`.
pub(crate) fn invariant(body: &MirBody, inside: &BTreeSet<i64>) -> BTreeSet<u32> {
    let mut written = BTreeSet::new();
    for block in &body.blocks {
        if !inside.contains(&block.at) {
            continue;
        }
        written.extend(
            block
                .ops
                .iter()
                .flat_map(|op| op.defines.iter().map(|value| value.id)),
        );
        written.extend(block.phis.iter().map(|phi| phi.result.id));
    }
    body.blocks
        .iter()
        .flat_map(|block| block.ops.iter())
        .flat_map(|op| {
            op.defines
                .iter()
                .chain(op.uses.iter())
                .map(|value| value.id)
        })
        .filter(|value| !written.contains(value))
        .collect()
}

/// Python's `basics(body, loop)`.
pub(crate) fn basics(body: &MirBody, loop_: &Loop) -> OrderedMap<u32, Affine> {
    // Python's `{block.at: block for block in body.blocks}` retains the last
    // duplicate address.  All later `at_of` reads use that exact view.
    let at_of = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    let inside = loop_
        .body
        .iter()
        .copied()
        .filter(|at| at_of.contains_key(at))
        .collect::<BTreeSet<_>>();
    let Some(header) = at_of.get(&loop_.header).copied() else {
        return OrderedMap::new();
    };
    let still = invariant(body, &inside);
    let mut made = BTreeMap::<u32, &Op>::new();
    for at in &inside {
        for operation in &at_of[at].ops {
            for value in &operation.defines {
                made.insert(value.id, operation);
            }
        }
    }

    let mut out = OrderedMap::new();
    for phi in &header.phis {
        let starts = phi
            .incoming
            .iter()
            .filter_map(|(where_, value)| (!inside.contains(where_)).then_some(*value))
            .collect::<Vec<_>>();
        if starts.is_empty() || starts.iter().any(|value| *value != starts[0]) {
            continue;
        }
        let mut steps = Vec::<Option<AffineOperand>>::new();
        let mut widths = BTreeSet::new();
        for (where_, value) in phi.incoming.iter() {
            if !inside.contains(where_) {
                continue;
            }
            let results = made
                .get(&value.id)
                .into_iter()
                .flat_map(|definition| definition.results.iter())
                .filter_map(|argument| match argument {
                    Arg::Held(held) if held.value == *value => Some(*held),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if results.len() != 1 {
                steps.push(None);
                continue;
            }
            let width = results[0].width;
            widths.insert(width);
            let root = _copied(results[0], &made);
            let step = _stepped(made.get(&root.value.id).copied(), phi.result.id, &still, &made);
            steps.push(step.filter(|step| step.width() == width));
        }
        if widths.len() == 1
            && !steps.is_empty()
            && steps[0].is_some()
            && steps.iter().all(|step| step == &steps[0])
        {
            let width = *widths.first().expect("one Python recurrence width");
            out.insert(
                phi.result.id,
                Affine {
                    value: phi.result.id,
                    start: AffineOperand::Held(Held {
                        value: starts[0],
                        width,
                    }),
                    step: steps[0].clone().expect("checked above"),
                    header: loop_.header,
                },
            );
        }
    }
    out
}

/// Python's `_copied(operand, made)`.
fn _copied(mut operand: Held, made: &BTreeMap<u32, &Op>) -> Held {
    let mut seen = BTreeSet::new();
    while !seen.contains(&operand.value.id) {
        seen.insert(operand.value.id);
        let Some(op) = made.get(&operand.value.id).copied() else {
            break;
        };
        if op.kind != Kind::Copy || !op.loads.is_empty() || !op.stores.is_empty() {
            break;
        }
        if op.args.len() != 1 || op.results.len() != 1 {
            break;
        }
        let (Arg::Held(source), Arg::Held(result)) = (&op.args[0], &op.results[0]) else {
            break;
        };
        if source.width != operand.width || result.width != operand.width {
            break;
        }
        operand = *source;
    }
    operand
}

/// Python's `_stepped(op, value, still, made)`.
fn _stepped(
    op: Option<&Op>,
    value: u32,
    still: &BTreeSet<u32>,
    made: &BTreeMap<u32, &Op>,
) -> Option<AffineOperand> {
    let op = op?;
    let (mut stepped, mut step) = crate::model::mir::stepping(op)?;
    if let Arg::Held(held) = stepped {
        stepped = Arg::Held(_copied(held, made));
    }
    if let Arg::Held(held) = step {
        step = Arg::Held(_copied(held, made));
    }
    if !matches!(&stepped, Arg::Held(held) if held.value.id == value) {
        if matches!(&step, Arg::Held(held) if held.value.id == value) {
            std::mem::swap(&mut stepped, &mut step);
        } else {
            return None;
        }
    }
    match step {
        Arg::Const(constant) => Some(AffineOperand::Const(constant)),
        Arg::Held(held) if still.contains(&held.value.id) => Some(AffineOperand::Held(held)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::codegen::machine::Operation;
    use crate::model::floating::{Format, Precision, Rounding, Semantics as FloatingSemantics};
    use crate::model::mir::{
        Arg, Const, FloatingOrigin, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Phi,
        Value,
    };
    use crate::model::mir_loops::Loop;

    use super::{
        _copied, AffineOperand, LoopShape, basics, canonical, invariant, test_only,
    };

    fn value(id: u32, at: i64) -> Value {
        Value::new(id, at)
    }

    fn op(at: i64, kind: Kind, defines: Vec<Value>, uses: Vec<Value>) -> Op {
        let mut op = Op::new(at, None, "", defines, uses);
        op.kind = kind;
        op
    }

    fn floating() -> FloatingSemantics {
        FloatingSemantics::new([], Format::Binary32, Precision::Exact, Rounding::None)
    }

    fn floating_origin() -> FloatingOrigin {
        FloatingOrigin {
            block: 0,
            sequence: vec![],
            at: 0,
            kind: Kind::Fadd,
            semantics: floating(),
            inputs: vec![],
            outputs: vec![],
            machine_inputs: vec![],
            machine_outputs: vec![],
        }
    }

    /// Direct Rust fixture for the source body used by
    /// `tests/test_induction_identity.py:body`.
    fn identity_body() -> (MirBody, Loop) {
        let start = Value {
            variable: 7,
            ..value(10, 0)
        };
        let counter = Value {
            variable: 7,
            ..value(11, 1)
        };
        let following = Value {
            variable: 7,
            ..value(12, 1)
        };
        let unrelated = Value {
            variable: 7,
            ..value(13, 1)
        };
        let answer = Value {
            variable: 8,
            ..value(14, 1)
        };
        let mut increment = op(1, Kind::Increment, vec![following], vec![counter]);
        increment.args = vec![Arg::Held(Held { value: counter, width: 2 })];
        increment.results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        let mut multiply = op(2, Kind::Mul, vec![answer], vec![unrelated]);
        multiply.args = vec![
            Arg::Held(Held {
                value: unrelated,
                width: 2,
            }),
            Arg::Const(Const::new(2, 2)),
        ];
        multiply.results = vec![Arg::Held(Held {
            value: answer,
            width: 2,
        })];
        let mut incoming = crate::model::mir::OrderedMap::new();
        incoming.insert(0, start);
        incoming.insert(1, following);
        (
            MirBody::new(
                0,
                vec![
                    MirBlock::new(0, vec![], vec![], vec![1]),
                    MirBlock::new(
                        1,
                        vec![Phi {
                            result: counter,
                            incoming,
                        }],
                        vec![increment, multiply],
                        vec![1, 2],
                    ),
                    MirBlock::new(2, vec![], vec![], vec![]),
                ],
            ),
            Loop {
                header: 1,
                latches: BTreeSet::from([1]),
                body: BTreeSet::from([1]),
            },
        )
    }

    /// Rust form of `tests/test_indvars.py:_symbolic_control_body`: its
    /// preheader, pre-tested header, one latch, and one exit are the exact
    /// shape induction proofs consume.
    fn symbolic_control_body() -> (MirBody, Loop) {
        let bound = value(1, 0);
        let control = value(4, 1);
        let control_next = value(7, 2);
        let flags = Value {
            flags: true,
            ..value(6, 1)
        };
        let mut branch = op(1, Kind::Branch, vec![], vec![flags]);
        branch.target = Some(3);
        let mut incoming = crate::model::mir::OrderedMap::new();
        incoming.insert(0, value(2, 0));
        incoming.insert(2, control_next);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(
                    0,
                    vec![],
                    vec![op(0, Kind::Load, vec![bound], vec![])],
                    vec![1],
                ),
                MirBlock::new(
                    1,
                    vec![Phi {
                        result: control,
                        incoming,
                    }],
                    vec![op(1, Kind::Sub, vec![flags], vec![control, bound]), branch],
                    vec![2, 3],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![
                        op(2, Kind::Add, vec![control_next], vec![control]),
                        op(2, Kind::Jump, vec![], vec![]),
                    ],
                    vec![1],
                ),
                MirBlock::new(3, vec![], vec![op(3, Kind::Return, vec![], vec![])], vec![]),
            ],
        );
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::from([2]),
            body: BTreeSet::from([1, 2]),
        };
        (body, loop_)
    }

    #[test]
    fn direct_induction_canonical_accepts_symbolic_control_shape() {
        let (body, loop_) = symbolic_control_body();
        assert_eq!(
            canonical(&body, &loop_),
            Some(LoopShape {
                preheader: 0,
                latch: 2,
                entered: 2,
                exit: 3
            })
        );
    }

    #[test]
    fn direct_induction_canonical_refuses_each_python_structural_exception() {
        let (body, loop_) = symbolic_control_body();
        let cases = [
            (
                Loop {
                    latches: BTreeSet::from([1, 2]),
                    ..loop_.clone()
                },
                body.clone(),
            ),
            (
                Loop {
                    header: 99,
                    ..loop_.clone()
                },
                body.clone(),
            ),
            (
                Loop {
                    latches: BTreeSet::from([99]),
                    ..loop_.clone()
                },
                body.clone(),
            ),
        ];
        for (loop_, body) in cases {
            assert_eq!(canonical(&body, &loop_), None);
        }

        let mut outside = body.clone();
        outside.blocks[0].succ = vec![1, 3];
        assert_eq!(canonical(&outside, &loop_), None);
        let mut two_preheaders = body.clone();
        two_preheaders
            .blocks
            .push(MirBlock::new(4, vec![], vec![], vec![1]));
        assert_eq!(canonical(&two_preheaders, &loop_), None);
        let mut latch = body.clone();
        latch.blocks[2].succ = vec![1, 3];
        assert_eq!(canonical(&latch, &loop_), None);
        let mut entry = body.clone();
        entry.blocks[1].succ = vec![2, 2, 3];
        assert_eq!(canonical(&entry, &loop_), None);
        let mut no_entry = body.clone();
        no_entry.blocks[1].succ = vec![3];
        assert_eq!(canonical(&no_entry, &loop_), None);
        let mut exits = body.clone();
        exits.blocks[1].succ = vec![2, 3, 4];
        assert_eq!(canonical(&exits, &loop_), None);
        let mut no_exit = body.clone();
        no_exit.blocks[1].succ = vec![2];
        assert_eq!(canonical(&no_exit, &loop_), None);
        let mut no_operations = body.clone();
        no_operations.blocks[1].ops.clear();
        assert_eq!(canonical(&no_operations, &loop_), None);
        let mut no_branch = body.clone();
        no_branch.blocks[1].ops.last_mut().unwrap().kind = Kind::Jump;
        assert_eq!(canonical(&no_branch, &loop_), None);
        let mut side_exit = body.clone();
        side_exit.blocks[1].succ = vec![2, 3];
        side_exit.blocks[2].succ = vec![4, 3];
        side_exit
            .blocks
            .push(MirBlock::new(4, vec![], vec![], vec![1]));
        let side_loop = Loop {
            header: 1,
            latches: BTreeSet::from([4]),
            body: BTreeSet::from([1, 2, 4]),
        };
        assert_eq!(canonical(&side_exit, &side_loop), None);
    }

    #[test]
    #[should_panic]
    fn direct_induction_canonical_preserves_python_unknown_body_keyerror() {
        let (body, loop_) = symbolic_control_body();
        let unknown_inside = Loop {
            body: BTreeSet::from([1, 2, 99]),
            ..loop_
        };
        let _ = canonical(&body, &unknown_inside);
    }

    #[test]
    fn direct_induction_test_only_matches_empty_and_flag_test_forms() {
        let empty = op(0, Kind::Nothing, vec![], vec![]);
        assert!(test_only(&empty));

        let flags = Value {
            flags: true,
            ..value(1, 0)
        };
        for kind in [Kind::Sub, Kind::And, Kind::Or] {
            let mut one = op(0, kind, vec![flags], vec![value(2, 0)]);
            one.args.push(Arg::Const(Const::new(7, 2)));
            assert!(test_only(&one));
        }
        assert!(!test_only(&op(0, Kind::Add, vec![flags], vec![])));
        assert!(!test_only(&op(0, Kind::Sub, vec![value(2, 0)], vec![])));
    }

    #[test]
    fn direct_induction_test_only_refuses_every_python_observable_field() {
        let empty = op(0, Kind::Nothing, vec![], vec![]);
        let flags = op(
            0,
            Kind::Sub,
            vec![Value {
                flags: true,
                ..value(1, 0)
            }],
            vec![],
        );
        let mut cases = Vec::new();

        let mut one = empty.clone();
        one.name = "data".into();
        cases.push(one);
        let mut one = empty.clone();
        one.defines.push(value(2, 0));
        cases.push(one);
        let mut one = empty.clone();
        one.uses.push(value(2, 0));
        cases.push(one);
        let mut one = empty.clone();
        one.args.push(Arg::Const(Const::new(1, 2)));
        cases.push(one);
        let mut one = empty.clone();
        one.results.push(Arg::Held(Held { value: value(2, 0), width: 2 }));
        cases.push(one);
        let mut one = empty.clone();
        one.loads.push(MemRef::new(None, 2));
        cases.push(one);
        let mut one = empty.clone();
        one.stores.push(MemRef::new(None, 2));
        cases.push(one);
        let mut one = empty.clone();
        one.merges.insert(value(2, 0), value(3, 0));
        cases.push(one);
        let mut one = empty.clone();
        one.op = Some(OpCode::Operation(Operation::Barrier));
        cases.push(one);
        let mut one = empty.clone();
        one.floating = Some(floating());
        cases.push(one);
        let mut one = empty.clone();
        one.stack = Some(0);
        cases.push(one);
        let mut one = empty;
        one.floating_origin = Some(floating_origin());
        cases.push(one);
        assert!(cases.iter().all(|one| !test_only(one)));

        let mut cases = Vec::new();
        let mut one = flags.clone();
        one.kind = Kind::Add;
        cases.push(one);
        let mut one = flags.clone();
        one.results.push(Arg::Held(Held { value: value(2, 0), width: 2 }));
        cases.push(one);
        let mut one = flags.clone();
        one.loads.push(MemRef::new(None, 2));
        cases.push(one);
        let mut one = flags.clone();
        one.stores.push(MemRef::new(None, 2));
        cases.push(one);
        let mut one = flags.clone();
        one.merges.insert(value(2, 0), value(3, 0));
        cases.push(one);
        let mut one = flags.clone();
        one.op = Some(OpCode::Operation(Operation::Barrier));
        cases.push(one);
        let mut one = flags.clone();
        one.floating = Some(floating());
        cases.push(one);
        let mut one = flags.clone();
        one.stack = Some(0);
        cases.push(one);
        let mut one = flags.clone();
        one.floating_origin = Some(floating_origin());
        cases.push(one);
        cases.push(op(0, Kind::Sub, vec![], vec![]));
        cases.push(op(0, Kind::Sub, vec![value(2, 0)], vec![]));
        assert!(cases.iter().all(|one| !test_only(one)));
    }

    #[test]
    fn direct_induction_invariant_returns_only_ids_not_written_inside() {
        let outside = value(1, 0);
        let inside_op = value(2, 1);
        let inside_phi = value(3, 1);
        let used_outside = value(4, 2);
        let mut incoming = crate::model::mir::OrderedMap::new();
        incoming.insert(0, outside);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(
                    0,
                    vec![],
                    vec![op(0, Kind::Copy, vec![outside], vec![])],
                    vec![1],
                ),
                MirBlock::new(
                    1,
                    vec![Phi {
                        result: inside_phi,
                        incoming,
                    }],
                    vec![op(1, Kind::Add, vec![inside_op], vec![outside])],
                    vec![2],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![op(
                        2,
                        Kind::Copy,
                        vec![],
                        vec![outside, inside_op, inside_phi, used_outside],
                    )],
                    vec![],
                ),
            ],
        );
        assert_eq!(
            invariant(&body, &BTreeSet::from([1])),
            BTreeSet::from([1, 4])
        );
    }

    #[test]
    fn direct_induction_basics_backedge_steps_the_exact_phi_value() {
        // Direct port of
        // tests/test_induction_identity.py:test_the_backedge_must_step_the_exact_phi_value.
        let (mut body, loop_) = identity_body();
        assert!(!basics(&body, &loop_).is_empty());
        let unrelated = match &body.blocks[1].ops[1].args[0] {
            Arg::Held(held) => *held,
            _ => unreachable!("identity fixture has a held multiply input"),
        };
        body.blocks[1].ops[0].args = vec![Arg::Held(unrelated)];
        body.blocks[1].ops[0].uses = vec![unrelated.value];
        assert!(basics(&body, &loop_).is_empty());
    }

    #[test]
    fn direct_induction_basics_accepts_counter_as_second_add_operand() {
        // Direct branch coverage for `_stepped`: Python permits the counter
        // as either add operand, then swaps the recurrence pair.
        let (mut body, loop_) = identity_body();
        let counter = body.blocks[1].phis[0].result;
        let following = body.blocks[1].ops[0].defines[0];
        body.blocks[1].ops[0].kind = Kind::Add;
        body.blocks[1].ops[0].args = vec![
            Arg::Const(Const::new(2, 2)),
            Arg::Held(Held {
                value: counter,
                width: 2,
            }),
        ];
        body.blocks[1].ops[0].results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        let found = basics(&body, &loop_);
        assert_eq!(
            found.get(&counter.id).map(|one| one.step.clone()),
            Some(AffineOperand::Const(Const::new(2, 2)))
        );
    }

    #[test]
    fn direct_induction_basics_accepts_an_invariant_held_step() {
        // Direct branch coverage for `_stepped`'s `step.value.id in still`.
        let (mut body, loop_) = identity_body();
        let counter = body.blocks[1].phis[0].result;
        let following = body.blocks[1].ops[0].defines[0];
        let increment = Value {
            variable: 12,
            ..value(40, 0)
        };
        body.blocks[0]
            .ops
            .push(op(0, Kind::Copy, vec![increment], vec![]));
        body.blocks[1].ops[0].kind = Kind::Add;
        body.blocks[1].ops[0].args = vec![
            Arg::Held(Held {
                value: counter,
                width: 2,
            }),
            Arg::Held(Held {
                value: increment,
                width: 2,
            }),
        ];
        body.blocks[1].ops[0].results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        let found = basics(&body, &loop_);
        assert_eq!(
            found.get(&counter.id).map(|one| one.step.clone()),
            Some(AffineOperand::Held(Held {
                value: increment,
                width: 2
            }))
        );
    }

    #[test]
    fn direct_induction_basics_refuses_a_loop_written_held_step() {
        // Direct branch coverage for `_stepped`: an otherwise matching held
        // step is not invariant when an in-loop operation defines it.
        let (mut body, loop_) = identity_body();
        let counter = body.blocks[1].phis[0].result;
        let following = body.blocks[1].ops[0].defines[0];
        let written = body.blocks[1].ops[1].defines[0];
        body.blocks[1].ops[0].kind = Kind::Add;
        body.blocks[1].ops[0].args = vec![
            Arg::Held(Held {
                value: counter,
                width: 2,
            }),
            Arg::Held(Held {
                value: written,
                width: 2,
            }),
        ];
        body.blocks[1].ops[0].results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        assert!(basics(&body, &loop_).is_empty());
    }

    #[test]
    fn direct_induction_basics_keeps_header_phi_insertion_order() {
        let (mut body, loop_) = identity_body();
        let start = Value {
            variable: 9,
            ..value(31, 0)
        };
        let counter = Value {
            variable: 9,
            ..value(32, 1)
        };
        let following = Value {
            variable: 9,
            ..value(33, 1)
        };
        let mut incoming = crate::model::mir::OrderedMap::new();
        incoming.insert(0, start);
        incoming.insert(1, following);
        let mut increment = op(1, Kind::Increment, vec![following], vec![counter]);
        increment.args = vec![Arg::Held(Held { value: counter, width: 2 })];
        increment.results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        body.blocks[1].phis.insert(
            0,
            Phi {
                result: counter,
                incoming,
            },
        );
        body.blocks[1].ops.insert(0, increment);
        assert_eq!(
            basics(&body, &loop_).keys().copied().collect::<Vec<_>>(),
            vec![counter.id, body.blocks[1].phis[1].result.id]
        );
    }

    #[test]
    fn direct_induction_basics_long_recurrence_keeps_its_width() {
        // Direct port of
        // tests/test_induction_identity.py:test_long_recurrence_keeps_its_width,
        // for both `copied` parameter values.
        for copied in [false, true] {
            let (mut body, loop_) = identity_body();
            // Python rebuilds this header as `ops = (update,)` before it
            // optionally makes `(update, copy)`.
            body.blocks[1].ops.truncate(1);
            let counter = body.blocks[1].phis[0].result;
            let following = body.blocks[1].ops[0].defines[0];
            body.blocks[1].ops[0].args = vec![Arg::Held(Held {
                value: counter,
                width: 4,
            })];
            body.blocks[1].ops[0].results = vec![Arg::Held(Held {
                value: following,
                width: 4,
            })];
            if copied {
                let temporary = Value {
                    variable: 7,
                    ..value(100, 1)
                };
                body.blocks[1].ops[0].defines = vec![temporary];
                body.blocks[1].ops[0].results = vec![Arg::Held(Held {
                    value: temporary,
                    width: 4,
                })];
                let mut copy = op(3, Kind::Copy, vec![following], vec![temporary]);
                copy.args = vec![Arg::Held(Held {
                    value: temporary,
                    width: 4,
                })];
                copy.results = vec![Arg::Held(Held {
                    value: following,
                    width: 4,
                })];
                body.blocks[1].ops.insert(1, copy);
            }
            let found = basics(&body, &loop_);
            let recurrence = found
                .get(&counter.id)
                .expect("the width-preserving recurrence remains affine");
            assert_eq!(
                recurrence.start,
                AffineOperand::Held(Held {
                    value: body.blocks[1].phis[0].incoming.get(&0).copied().unwrap(),
                    width: 4
                })
            );
            assert_eq!(recurrence.step, AffineOperand::Const(Const::new(1, 4)));
        }
    }

    #[test]
    fn direct_induction_basics_every_incoming_path_agrees_on_the_recurrence() {
        // Direct port of
        // tests/test_induction_identity.py:test_every_incoming_path_agrees_on_the_recurrence.
        for mismatch in ["start", "step", "unchanged", "none"] {
            let (mut body, mut loop_) = identity_body();
            let counter = body.blocks[1].phis[0].result;
            let start = *body.blocks[1].phis[0]
                .incoming
                .get(&0)
                .expect("identity phi has its preheader input");
            let following = Value {
                variable: 7,
                ..value(20, 3)
            };
            let mut step = body.blocks[1].ops[0].clone();
            step.at = 3;
            step.defines = vec![following];
            step.results = vec![Arg::Held(Held {
                value: following,
                width: 2,
            })];
            if mismatch == "step" {
                step.kind = Kind::Decrement;
            }
            let phi = &mut body.blocks[1].phis[0];
            phi.incoming.insert(
                4,
                if mismatch == "start" {
                    Value {
                        variable: 7,
                        ..value(21, 4)
                    }
                } else {
                    start
                },
            );
            phi.incoming.insert(
                3,
                if mismatch == "unchanged" {
                    counter
                } else {
                    following
                },
            );
            body.blocks[1].succ = vec![1, 2, 3];
            body.blocks.push(MirBlock::new(3, vec![], vec![step], vec![1]));
            body.blocks.push(MirBlock::new(4, vec![], vec![], vec![1]));
            loop_.body = BTreeSet::from([1, 3]);
            loop_.latches = BTreeSet::from([1, 3]);
            assert_eq!(!basics(&body, &loop_).is_empty(), mismatch == "none");
        }
    }

    #[test]
    fn direct_induction_basics_copied_requires_width_preservation() {
        // Direct helper port of the `_copied` premise in
        // tests/test_induction_identity.py:test_only_width_preserving_copies_carry_the_recurrence.
        for (width, expected_source) in [(2, true), (4, false)] {
            let (body, _) = identity_body();
            let counter = body.blocks[1].phis[0].result;
            let copied = match &body.blocks[1].ops[1].args[0] {
                Arg::Held(held) => held.value,
                _ => unreachable!("identity fixture has a held multiply input"),
            };
            let mut copy = op(1, Kind::Copy, vec![copied], vec![counter]);
            copy.args = vec![Arg::Held(Held {
                value: counter,
                width,
            })];
            copy.results = vec![Arg::Held(Held {
                value: copied,
                width: 2,
            })];
            let made = BTreeMap::from([(copied.id, &copy)]);
            let root = _copied(Held { value: copied, width: 2 }, &made);
            assert_eq!(root.value == counter, expected_source);
            assert_eq!(root.width, 2);
        }
    }

    #[test]
    fn direct_induction_basics_copied_stops_at_every_python_boundary() {
        // Focused coverage of every refusal branch in the direct port of
        // qbopt.analysis.induction:_copied.
        let source = value(1, 0);
        let result = value(2, 1);
        let operand = Held {
            value: result,
            width: 2,
        };
        let mut copy = op(1, Kind::Copy, vec![result], vec![source]);
        copy.args = vec![Arg::Held(Held {
            value: source,
            width: 2,
        })];
        copy.results = vec![Arg::Held(operand)];
        let follows = |copy: &Op| {
            let made = BTreeMap::from([(result.id, copy)]);
            _copied(operand, &made)
        };
        assert_eq!(follows(&copy).value, source);

        let mut non_copy = copy.clone();
        non_copy.kind = Kind::Add;
        assert_eq!(follows(&non_copy), operand);
        let mut memory = copy.clone();
        memory.loads.push(MemRef::new(None, 2));
        assert_eq!(follows(&memory), operand);
        let mut arity = copy.clone();
        arity.args.push(Arg::Const(Const::new(1, 2)));
        assert_eq!(follows(&arity), operand);
        let mut type_mismatch = copy.clone();
        type_mismatch.args = vec![Arg::Const(Const::new(1, 2))];
        assert_eq!(follows(&type_mismatch), operand);
        let mut source_width = copy.clone();
        source_width.args = vec![Arg::Held(Held { value: source, width: 4 })];
        assert_eq!(follows(&source_width), operand);
        let mut result_width = copy.clone();
        result_width.results = vec![Arg::Held(Held { value: result, width: 4 })];
        assert_eq!(follows(&result_width), operand);

        let mut cycle = copy;
        cycle.args = vec![Arg::Held(operand)];
        assert_eq!(follows(&cycle), operand);
    }
}
