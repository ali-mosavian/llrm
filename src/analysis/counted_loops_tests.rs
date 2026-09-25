//! Port of the MIR cases of `tests/test_counted_loops.py`: `induction::counted`,
//! the one trip-count proof, against what the loops it names actually do.
//!
//! Skipped, needing the QB or C frontends: the runtime-lower-bound sumThree
//! cases and the `pairs` C cases.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use num_bigint::BigInt;

use super::{_ASCENDING, _DESCENDING, counted};
use crate::analysis::loops;
use crate::model::execute::{ExecutionError, Memory, Outcome, run};
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value};
use crate::objectfile::module::{Addr, Space};
use crate::optimize::{loopexit, rotate};
use crate::support::hash::IndexMap;

const GIVEN: Value = Value { id: 99, at: 0, flags: false, variable: 99, version: 1 };

fn op(at: i64, operation: Operation, name: &str, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
    Op { kind, ..Op::new(at, OpCode::Operation(operation), name, defines, uses) }
}

fn held(value: Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

fn constant(n: i64, width: u32) -> Arg {
    Arg::Const(Const::new(n, width))
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Shape {
    posttested: bool,
    stepped: bool,
    mirrored: bool,
    width: u32,
    zero_test: bool,
    unobserved: bool,
    pub(crate) split: bool,
}

pub(crate) fn shaped(shape: &str) -> Shape {
    Shape { posttested: shape != "pre", stepped: shape == "post-stepped", width: 1, ..Shape::default() }
}

/// `i = start` stepping by `step` while `i test bound`, returning trips and the last trip's `i`.
///
/// `_loop` in the Python: a `start` of None is `GIVEN`, live in; an
/// unobserved `i` is read only by its test and step; `split` puts a
/// post-tested loop's test in a latch block of its own.
pub(crate) fn _loop(start: Option<i64>, bound: i64, test: Kind, step: i64, shape: Shape) -> MirBody {
    let width = shape.width;
    let mut serial = 0;
    let mut value = |at: i64, flags: bool| {
        serial += 1;
        Value { id: serial, at, flags, variable: serial, version: 1 }
    };
    let (body_at, exit_at) = if shape.posttested { (1, 2) } else { (2, 3) };
    let control = if shape.split { 4 } else { 1 };
    let latch = if shape.posttested { control } else { body_at };
    let (seed, zero, counter, trips, seen) = (value(0, false), value(0, false), value(1, false), value(1, false), value(1, false));
    let (following, counted_up, saw, flags) =
        (value(body_at, false), value(body_at, false), value(body_at, false), value(1, true));
    let tested = Held { value: if shape.stepped { following } else { counter }, width };
    let args = if shape.zero_test {
        vec![Arg::Held(tested), Arg::Held(tested)]
    } else if shape.mirrored {
        vec![constant(bound, width), Arg::Held(tested)]
    } else {
        vec![Arg::Held(tested), constant(bound, width)]
    };
    let kind = if shape.zero_test { Kind::Or } else { Kind::Sub };
    let compare = Op { args, ..op(control, Operation::Compare, "cmp", vec![flags], vec![tested.value], kind) };
    let continuing = if shape.mirrored { mir::MIRRORED(test).unwrap() } else { test };
    let branch = Op {
        test: Some(if shape.posttested { continuing } else { mir::NEGATED(continuing).unwrap() }),
        target: Some(if shape.posttested { 1 } else { exit_at }),
        ..op(control, Operation::Branch, "", vec![], vec![flags], Kind::Branch)
    };
    let mut trip = vec![
        mir::computed(body_at, Kind::Add, counted_up, vec![held(trips, 2), constant(1, 2)], 2),
        mir::computed(body_at, Kind::Copy, saw, vec![held(counter, width)], width),
        mir::computed(body_at, Kind::Add, following, vec![held(counter, width), constant(step, width)], width),
    ];
    let mut phis = vec![
        Phi { result: counter, incoming: OrderedMap::from_iter([(0, seed), (latch, following)]) },
        Phi { result: trips, incoming: OrderedMap::from_iter([(0, zero), (latch, counted_up)]) },
        Phi { result: seen, incoming: OrderedMap::from_iter([(0, seed), (latch, saw)]) },
    ];
    let mut left = if shape.posttested {
        vec![Held { value: counted_up, width: 2 }, Held { value: saw, width }]
    } else {
        vec![Held { value: trips, width: 2 }, Held { value: seen, width }]
    };
    if shape.unobserved {
        trip.remove(1);
        phis.truncate(2);
        left.truncate(1);
    }
    // Closed over the loop, as every pass after LCSSA sees it.
    let closed = left
        .iter()
        .map(|one| Phi { result: value(exit_at, false), incoming: OrderedMap::from_iter([(control, one.value)]) })
        .collect::<Vec<_>>();
    let returned = Op {
        args: closed.iter().zip(&left).map(|(phi, one)| held(phi.result, one.width)).collect(),
        ..op(exit_at, Operation::Return, "", vec![], vec![], Kind::Return)
    };
    let seeded = match start {
        None => held(GIVEN, width),
        Some(start) => constant(start, width),
    };
    let entry = MirBlock::new(
        0,
        vec![],
        vec![mir::computed(0, Kind::Copy, seed, vec![seeded], width), mir::computed(0, Kind::Copy, zero, vec![constant(0, 2)], 2)],
        vec![1],
    );
    let blocks = if shape.posttested && shape.split {
        let jump = Op { target: Some(4), ..op(1, Operation::Jump, "", vec![], vec![], Kind::Jump) };
        trip.push(jump);
        vec![
            entry,
            MirBlock::new(1, phis, trip, vec![4]),
            MirBlock::new(2, closed, vec![returned], vec![]),
            MirBlock::new(4, vec![], vec![compare, branch], vec![1, 2]),
        ]
    } else if shape.posttested {
        trip.extend([compare, branch]);
        vec![entry, MirBlock::new(1, phis, trip, vec![1, 2]), MirBlock::new(2, closed, vec![returned], vec![])]
    } else {
        let jump = Op { target: Some(1), ..op(2, Operation::Jump, "", vec![], vec![], Kind::Jump) };
        trip.push(jump);
        vec![
            entry,
            MirBlock::new(1, phis, vec![compare, branch], vec![2, 3]),
            MirBlock::new(2, vec![], trip, vec![1]),
            MirBlock::new(3, closed, vec![returned], vec![]),
        ]
    };
    MirBody { sealed: true, ..MirBody::new(0, blocks) }
}

fn executed(body: &MirBody, given: Option<i64>, limit: u64) -> Result<Outcome, ExecutionError> {
    let values = given.map(|n| IndexMap::from_iter([(GIVEN, BigInt::from(n))])).unwrap_or_default();
    run(body, &values, &Memory::default(), None, limit, &BTreeMap::new())
}

fn only_loop(body: &MirBody) -> loops::Loop {
    let found = loops::loops(&body.blocks, Some(body.entry));
    let [one] = &found[..] else { panic!("one loop") };
    one.clone()
}

fn _signed(n: &BigInt, width: u32) -> BigInt {
    if n >> (8 * width - 1) != BigInt::from(0_u8) { n - (BigInt::from(1_u8) << (8 * width)) } else { n.clone() }
}

const TESTS: [Kind; 9] = [
    Kind::Lt,
    Kind::Le,
    Kind::Below,
    Kind::BelowEq,
    Kind::Gt,
    Kind::Ge,
    Kind::Above,
    Kind::AboveEq,
    Kind::Ne,
];
const ENDS: [i64; 6] = [0, 1, 0x7F, 0x80, 0xFE, 0xFF];

/// The proof, over every byte loop of these ends and tests, against running it.
///
/// A loop the executor sees end has its exact count, or no proof; one that
/// never ends has no proof. The last trip's counter is `last` wherever given.
#[test]
fn test_every_counted_loop_runs_its_proved_trips() {
    for shape in ["pre", "post", "post-stepped"] {
        for step in [1, -1, 3, -3] {
            let mut proved = BTreeSet::new();
            for test in TESTS {
                for start in ENDS {
                    for bound in ENDS {
                        for mirrored in [false, true] {
                            let body = _loop(Some(start), bound, test, step, Shape { mirrored, ..shaped(shape) });
                            let loop_ = only_loop(&body);
                            let proofs = counted(&Rc::new(body.clone()), &loop_, None, false);
                            let where_ = (shape, step, test, start, bound, mirrored);
                            let Ok(outcome) = executed(&body, None, 1_700) else {
                                assert!(proofs.is_empty(), "{where_:?}");
                                continue;
                            };
                            let [proof] = &proofs[..] else {
                                assert!(proofs.is_empty(), "{where_:?}");
                                continue;
                            };
                            let (trips, seen) = (&outcome.returned[0], &outcome.returned[1]);
                            assert!(proof.count.as_ref() == Some(trips) && proof.test == test, "{where_:?} {trips}");
                            if proof.last.is_some() {
                                assert_eq!(
                                    (proof.first.clone(), proof.last.clone()),
                                    (Some(_signed(&BigInt::from(start), 1)), Some(_signed(seen, 1))),
                                    "{where_:?}"
                                );
                            }
                            proved.insert(test);
                        }
                    }
                }
            }
            // Every test whose direction the step can end is proved somewhere.
            let ending =
                TESTS.into_iter().filter(|test| *test == Kind::Ne || _ASCENDING(*test) == (step > 0)).collect::<BTreeSet<_>>();
            assert_eq!(proved, ending, "{shape} {step}");
            assert!(TESTS.iter().all(|test| *test == Kind::Ne || _ASCENDING(*test) != _DESCENDING(*test)));
        }
    }
}

/// loopexit assumed a header exit, and a post-tested loop leaves from its latch.
#[test]
fn test_exit_values_hold_for_every_counted_shape() {
    for split in [false, true] {
        for shape in ["pre", "post", "post-stepped"] {
            let shape = Shape { width: 2, split: split && shape != "pre", ..shaped(shape) };
            let body = Rc::new(_loop(Some(3), 10, Kind::Lt, 2, shape));
            let evaluated = loopexit::evaluated(&body).expect("evaluates");
            assert_eq!(executed(&evaluated, None, 1_000_000).unwrap().returned, executed(&body, None, 1_000_000).unwrap().returned);
        }
    }
}

/// `counted` reached `derived` for a memory bound, whose quotient rule asked `counted`: unbounded recursion.
#[test]
fn test_a_loop_dividing_its_counter_is_counted_without_asking_itself() {
    let mut body = _loop(None, 5, Kind::Lt, 1, Shape { width: 2, unobserved: true, ..shaped("pre") });
    let loop_ = only_loop(&body);
    let trips = body.block(loop_.header).unwrap().phis[1].result;
    let (quotient, remainder) = (Value { variable: 90, ..Value::new(90, 2) }, Value { variable: 91, ..Value::new(91, 2) });
    let divide = Op {
        args: vec![held(trips, 2), constant(1, 2)],
        results: vec![held(quotient, 2), held(remainder, 2)],
        ..op(2, Operation::Binary, "", vec![quotient, remainder], vec![trips], Kind::Divmod)
    };
    let block = body.blocks.iter_mut().find(|block| block.at == 2).unwrap();
    block.ops.insert(0, divide);

    let proofs = counted(&Rc::new(body), &loop_, None, true);
    let [proof] = &proofs[..] else { panic!("one proof") };
    assert_eq!(proof.count, None);
}

/// The skip guard kept an `or i,i` test's kind for `0 or start`: a countdown from 5 ran no trips.
#[test]
fn test_a_zero_tested_counter_from_a_runtime_start_is_skipped_exactly() {
    for (test, step) in [(Kind::Gt, -1), (Kind::Ne, 1), (Kind::Ne, -1)] {
        let body = Rc::new(_loop(None, 0, test, step, Shape { width: 2, zero_test: true, unobserved: true, ..shaped("pre") }));
        let loop_ = only_loop(&body);
        let proofs = counted(&body, &loop_, None, true);
        let [proof] = &proofs[..] else { panic!("one proof") };
        assert_eq!(proof.count, None); // symbolic, so rotation places a guard
        let rotated = rotate::entered(&body).expect("rotates");
        assert_ne!(*rotated, *body);
        for start in [0, 1, 5, 0x7FFF, 0xFFFB] {
            // Counting down past the sign, GT never ends within the limit.
            let Ok(expected) = executed(&body, Some(start), 500_000) else { continue };
            assert_eq!(executed(&rotated, Some(start), 500_000).unwrap().returned, expected.returned, "{test:?} {start}");
        }
    }
}

/// IVARM lost its ten-trip proof when IndVarSimplify changed `<= 10` to `!= 37`.
#[test]
fn test_a_not_equal_loop_knows_its_last_trip_only_without_wrapping() {
    let cases: [(i64, i64, i64, Option<i64>); 8] = [
        (7, 3, 37, Some(34)),
        (37, -3, 7, Some(10)),
        (0, 1, 32767, Some(32766)),
        (0, -1, -32768, Some(-32767)),
        (7, 3, 38, None), // reached only after wrapping
        (7, -3, 37, None),
        (7, 3, 7, None),         // no trip
        (32767, 1, -32768, None), // its exit value wraps
    ];
    for (start, step, bound, last) in cases {
        let body = _loop(Some(start), bound, Kind::Ne, step, Shape { width: 2, ..shaped("pre") });
        let loop_ = only_loop(&body);
        let proofs = counted(&Rc::new(body), &loop_, None, false);
        let [proof] = &proofs[..] else { panic!("one proof") };
        assert_eq!(proof.last, last.map(BigInt::from), "{start} {step} {bound}");
    }
}

/// `for i = 0 to n: load a[i]` over an unknown `n`: only the access can bound its trips.
fn _indexing_loop(inbounds: bool) -> MirBody {
    let at = |n: u32, at: i64| Value { variable: n, version: 1, ..Value::new(n, at) };
    let (seed, limit, counter, following, loaded) = (at(1, 0), at(2, 0), at(3, 1), at(4, 2), at(5, 2));
    let flags = Value { flags: true, ..at(6, 1) };
    let segment = |disp: i64| Addr { index: 1, ..Addr::new(Space::Segment, disp) };
    let cell = MemRef {
        base: Some(counter),
        space: Some(Space::Segment),
        base_width: 2,
        inbounds,
        ..MemRef::new(Some(segment(0)), 1)
    };
    let compare = Op {
        args: vec![held(counter, 2), held(limit, 2)],
        ..op(1, Operation::Compare, "cmp", vec![flags], vec![counter, limit], Kind::Sub)
    };
    let branch =
        Op { test: Some(Kind::Above), target: Some(3), ..op(1, Operation::Branch, "", vec![], vec![flags], Kind::Branch) };
    let jump = Op { target: Some(1), ..op(2, Operation::Jump, "", vec![], vec![], Kind::Jump) };
    let returned = op(3, Operation::Return, "", vec![], vec![], Kind::Return);
    let entry = vec![
        mir::computed(0, Kind::Copy, seed, vec![constant(0, 2)], 2),
        mir::computed(0, Kind::Load, limit, vec![Arg::Cell(Cell { r#ref: MemRef::new(Some(segment(2)), 2) })], 2),
    ];
    let step = vec![
        mir::computed(2, Kind::Load, loaded, vec![Arg::Cell(Cell { r#ref: cell })], 1),
        mir::computed(2, Kind::Increment, following, vec![held(counter, 2)], 2),
        jump,
    ];
    let phi = Phi { result: counter, incoming: OrderedMap::from_iter([(0, seed), (2, following)]) };
    MirBody {
        sealed: true,
        ..MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], entry, vec![1]),
                MirBlock::new(1, vec![phi], vec![compare, branch], vec![2, 3]),
                MirBlock::new(2, vec![], step, vec![1]),
                MirBlock::new(3, vec![], vec![returned], vec![]),
            ],
        )
    }
}

/// Raised BC's `a[i]` may wrap its 16-bit offset, yet it bounded `i <= n` as if it could not.
#[test]
fn test_only_a_promised_access_bounds_an_inclusive_loop() {
    for (inbounds, maximum) in [(true, Some(0x10000)), (false, None)] {
        let body = _indexing_loop(inbounds);
        let loop_ = only_loop(&body);
        let maxima = counted(&Rc::new(body), &loop_, None, true).into_iter().map(|proof| proof.maximum).collect::<Vec<_>>();
        assert_eq!(maxima, maximum.map(|maximum| vec![Some(BigInt::from(maximum))]).unwrap_or_default(), "{inbounds}");
    }
}
