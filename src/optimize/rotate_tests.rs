//! Port of the direct `rotate` tests: tests/test_countdown.py and
//! tests/test_rotate.py.
//!
//! Skipped:
//! - tests/test_countdown.py::test_dynamic_frequencies_do_not_charge_a_rotated_entry_guard_as_fifty_fifty:
//!   `tools.quality`, not rotate.
//! - `rotate.rotated` asserts inside tests/test_rewind.py and tests/test_indvars.py
//!   belong to those modules' tests.

use std::collections::BTreeSet;

use super::*;
use crate::model::ir::Operation;
use crate::model::mir::{Cell, MemRef};
use crate::objectfile::module::Space;

fn op(at: i64, operation: Operation, name: &str, defines: Vec<Value>, uses: Vec<Value>) -> Op {
    Op::new(at, OpCode::Operation(operation), name, defines, uses)
}

fn counted_loop(observed: bool) -> MirBody {
    let seed = Value {
        variable: 1,
        version: 1,
        ..Value::new(1, 0)
    };
    let bound = Value {
        variable: 2,
        version: 1,
        ..Value::new(2, 0)
    };
    let counter = Value {
        variable: 1,
        version: 2,
        ..Value::new(3, 1)
    };
    let following = Value {
        variable: 1,
        version: 3,
        ..Value::new(4, 2)
    };
    let flags = Value {
        flags: true,
        variable: 3,
        version: 1,
        ..Value::new(5, 1)
    };
    let source = MemRef {
        space: Some(Space::Frame),
        ..MemRef::new(None, 2)
    };
    let sink = MemRef {
        space: Some(Space::Segment),
        ..MemRef::new(None, 2)
    };
    let initialize = Op {
        kind: Kind::Copy,
        args: vec![Arg::Const(Const::new(0, 2))],
        results: vec![Arg::Held(Held {
            value: seed,
            width: 2,
        })],
        ..op(0, Operation::Move, "", vec![seed], vec![])
    };
    let load = Op {
        loads: vec![source.clone()],
        kind: Kind::Load,
        args: vec![Arg::Cell(Cell { r#ref: source })],
        results: vec![Arg::Held(Held {
            value: bound,
            width: 2,
        })],
        ..op(0, Operation::Move, "", vec![bound], vec![])
    };
    let compare = Op {
        kind: Kind::Sub,
        args: vec![
            Arg::Held(Held {
                value: counter,
                width: 2,
            }),
            Arg::Held(Held {
                value: bound,
                width: 2,
            }),
        ],
        ..op(
            1,
            Operation::Compare,
            "cmp",
            vec![flags],
            vec![counter, bound],
        )
    };
    let branch = Op {
        kind: Kind::Branch,
        test: Some(Kind::AboveEq),
        target: Some(3),
        ..op(1, Operation::Branch, "", vec![], vec![flags])
    };
    let store = Op {
        stores: vec![sink.clone()],
        kind: Kind::Store,
        args: vec![Arg::Held(Held {
            value: counter,
            width: 2,
        })],
        results: vec![Arg::Cell(Cell { r#ref: sink })],
        ..op(2, Operation::Move, "", vec![], vec![counter])
    };
    let increment = Op {
        kind: Kind::Increment,
        args: vec![Arg::Held(Held {
            value: counter,
            width: 2,
        })],
        results: vec![Arg::Held(Held {
            value: following,
            width: 2,
        })],
        ..op(2, Operation::Unary, "", vec![following], vec![counter])
    };
    let jump = Op {
        kind: Kind::Jump,
        target: Some(1),
        ..op(2, Operation::Jump, "", vec![], vec![])
    };
    let returned = Op {
        kind: Kind::Return,
        ..op(3, Operation::Return, "", vec![], vec![])
    };
    let latch = if observed {
        vec![store, increment, jump]
    } else {
        vec![increment, jump]
    };
    MirBody {
        sealed: true,
        ..MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![initialize, load], vec![1]),
                MirBlock::new(
                    1,
                    vec![Phi {
                        result: counter,
                        incoming: OrderedMap::from_iter([(0, seed), (2, following)]),
                    }],
                    vec![compare, branch],
                    vec![2, 3],
                ),
                MirBlock::new(2, vec![], latch, vec![1]),
                MirBlock::new(3, vec![], vec![returned], vec![]),
            ],
        )
    }
}

/// C floats retained `add/cmp/jb` in its ten-trip hot path.
///
/// Clang's canonical form tests the dynamic iteration count once before the
/// loop, then ends every executed trip with `dec/jne`.  The entry test is
/// essential: `bench_floats(0)` must still execute the body zero times.
/// Require the semantic shape rather than only looking for a `dec` in the
/// final listing, so dropping the zero-trip guard cannot satisfy the test.
#[test]
fn test_dead_dynamic_counter_counts_down_on_the_step_flags_after_a_zero_trip_guard() {
    let body = entered(&Rc::new(MirBody::clone(&counted_loop(false)))).unwrap();
    let found = loops::loops(&body.blocks, Some(body.entry));
    let [loop_] = found.as_slice() else {
        panic!("{found:?}")
    };
    let inside = loop_.body.iter().copied().collect::<BTreeSet<_>>();
    let decrements = body
        .blocks
        .iter()
        .filter(|block| inside.contains(&block.at))
        .flat_map(|block| &block.ops)
        .filter(|op| op.kind == Kind::Decrement)
        .collect::<Vec<_>>();
    assert_eq!(decrements.len(), 1);
    let flags = decrements[0]
        .defines
        .iter()
        .copied()
        .filter(|value| value.flags)
        .collect::<BTreeSet<_>>();
    let backedges = body
        .blocks
        .iter()
        .filter(|block| inside.contains(&block.at))
        .flat_map(|block| &block.ops)
        .filter(|op| {
            op.kind == Kind::Branch && op.target.is_some_and(|target| inside.contains(&target))
        })
        .collect::<Vec<_>>();
    assert!(backedges.len() == 1 && backedges[0].test == Some(Kind::Ne));
    assert!(backedges[0].uses.iter().any(|value| flags.contains(value)));

    let predecessors = loops::predecessors(&body.blocks);
    let entries = predecessors[&loop_.header]
        .iter()
        .copied()
        .filter(|at| !inside.contains(at))
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    let guard = body.block(entries[0]).unwrap();
    assert!(guard.succ.len() == 2 && guard.succ.iter().any(|at| !inside.contains(at)));
    let last = guard.ops.last().unwrap();
    assert!(last.kind == Kind::Branch && last.test == Some(Kind::Eq));
}

/// Replacing an index stored by the body with trips-remaining changes the program.
#[test]
fn test_countdown_refuses_an_observed_source_counter() {
    let body = counted_loop(true);

    assert_eq!(entered(&Rc::new(MirBody::clone(&body))).unwrap(), Rc::new(body));
}

/// Re-deriving the moved phis by variable renamed main's exit copy of a
/// call's answer to the loop counter; decide folded the exit away and the
/// segment was refused.
#[test]
#[ignore = "fails in Python too: 0x03f7: 45 bytes between the ops are not instructions"]
fn test_entering_mains_first_loop_at_its_body_keeps_the_code_after_it() {
    crate::support::testing::emitted_lir("fixtures/regressions/qbdemo-fil2.obj");
}

/// Entered at its body, harr's inner loop branched to a copy block placed
/// after the procedure and jumped back: two jumps a pass.
#[test]
fn test_a_back_edge_keeps_its_copies_in_the_latch() {
    use crate::wholeseg::{Emission, Watched};
    let mut body: crate::support::hash::IndexMap<String, crate::model::lir::LirBody> = Default::default();
    let mut watch = |stage: &str, name: Option<&str>, state: Watched<'_>| {
        if let (true, Some(name), Watched::Lir(state)) = (stage == "peephole", name, state) {
            body.insert(name.to_owned(), state.clone());
        }
    };
    let data = crate::support::testing::data("fixtures/omf/harr-v-g3.obj");
    let result = crate::support::testing::emitted_watching(&data, Some(&mut watch));
    assert_eq!(result.outcome, Emission::Lir, "{}", result.reason);
    assert!(!body.is_empty());
    let split: Vec<String> = body
        .iter()
        .flat_map(|(name, state)| {
            let end = (state.entry + 1) << 32;
            state.blocks.iter().filter(move |block| block.at >= end).map(move |block| format!("{name} {:#x}", block.at))
        })
        .collect();
    assert!(split.is_empty(), "{split:?}");
}
