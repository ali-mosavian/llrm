//! Control-only facts for Python MIR induction analysis.
//!
//! Direct port of `qbopt.analysis.induction:LoopShape`, `canonical`,
//! `invariant`, and `test_only`.  This module deliberately does not invent a
//! portable-IR loop abstraction: Python MIR is the current stage contract.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::mir::{Kind, MirBody, Op};
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::codegen::machine::Operation;
    use crate::model::floating::{Format, Precision, Rounding, Semantics as FloatingSemantics};
    use crate::model::mir::{
        Arg, Const, FloatingOrigin, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Phi,
        Value,
    };
    use crate::model::mir_loops::Loop;

    use super::{LoopShape, canonical, invariant, test_only};

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
}
