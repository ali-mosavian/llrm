//! Ports of `tests/test_ranges.py`, `tests/test_edge_ranges.py` and
//! `tests/test_unsigned_edge_ranges.py`.
//!
//! Skipped, needing the corpus raise and `transform.applied`:
//! `test_fpdeep_one_based_index_has_a_bounded_byte_offset`,
//! `test_addrm_long_array_value_keeps_counter_bounds`,
//! `test_nbody_scaled_index_is_bounded_only_inside_its_loop`,
//! `test_rngarm_writes_its_counter_only_after_the_loop`.

use std::rc::Rc;
use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use super::{_computed, _recurrence_span, Interval, bounded, on_edge};
use crate::analysis::regions::overlapping;
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value};
use crate::objectfile::module::{Addr, Space};

fn value(id: u32, at: i64) -> Value {
    Value::new(id, at)
}

fn interval(low: i64, high: i64, width: u32) -> Interval {
    Interval {
        low: low.into(),
        high: high.into(),
        width,
    }
}

fn operation(at: i64, operation: Operation, kind: Kind, defines: Vec<Value>, uses: Vec<Value>) -> Op {
    let mut made = Op::new(at, OpCode::Operation(operation), "", defines, uses);
    made.kind = kind;
    made
}

fn word(value: Value) -> Arg {
    Arg::Held(Held { value, width: 2 })
}

/// `tests/test_edge_ranges.py:guarded_loop`.
pub(crate) fn guarded_loop() -> MirBody {
    let (start, counter, advanced, offset) = (value(1, 0), value(2, 10), value(3, 40), value(6, 30));
    let compare = |at: i64, bound: i64, yes: i64, no: i64| {
        let flags = Value {
            flags: true,
            ..value(u32::try_from(at + 10).unwrap(), at)
        };
        let mut test = operation(at, Operation::Compare, Kind::Sub, vec![flags], vec![counter]);
        test.args = vec![word(counter), Arg::Const(Const::new(bound, 2))];
        let mut branch = operation(at + 1, Operation::Branch, Kind::Branch, vec![], vec![flags]);
        branch.test = Some(Kind::Lt);
        branch.target = Some(yes);
        MirBlock::new(at, vec![], vec![test, branch], vec![yes, no])
    };
    let mut initial = operation(0, Operation::Move, Kind::Copy, vec![start], vec![]);
    initial.args = vec![Arg::Const(Const::new(0, 2))];
    initial.results = vec![word(start)];
    let mut header = compare(10, 10, 20, 50);
    let mut incoming = OrderedMap::new();
    incoming.insert(0, start);
    incoming.insert(40, advanced);
    header.phis = vec![Phi {
        result: counter,
        incoming,
    }];
    let mut scaled = operation(30, Operation::Binary, Kind::Mul, vec![offset], vec![counter]);
    scaled.args = vec![word(counter), Arg::Const(Const::new(2, 2))];
    scaled.results = vec![word(offset)];
    let mut step = operation(40, Operation::Unary, Kind::Increment, vec![advanced], vec![counter]);
    step.args = vec![word(counter)];
    step.results = vec![word(advanced)];
    MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![initial], vec![10]),
            header,
            compare(20, 4, 30, 40),
            MirBlock::new(30, vec![], vec![scaled], vec![40]),
            MirBlock::new(40, vec![], vec![step], vec![10]),
            MirBlock::new(50, vec![], vec![], vec![]),
        ],
    )
}

#[test]
fn test_guard_refines_subscript_without_leaking_to_the_join() {
    // `tests/test_edge_ranges.py`: i<4 bounds a word-array offset to 0..6, not the whole loop's 0..18.
    let body = guarded_loop();
    let counter = body.blocks[1].phis[0].result;
    let offset = value(6, 30);
    let known = bounded(&Rc::new(MirBody::clone(&body))).unwrap();

    assert_eq!(known[&30][&counter], interval(0, 3, 2));
    assert_eq!(known[&30][&offset], interval(0, 6, 2));
    assert_eq!(known[&40][&counter], interval(0, 9, 2));
    assert!(!known.get(&50).is_some_and(|facts| facts.contains_key(&counter)));
}

#[test]
fn test_signed_comparison_edges() {
    // `tests/test_edge_ranges.py`, every parametrized case.
    for (test, successor, (low, high)) in [
        (Kind::Lt, 30, (0, 3)),
        (Kind::Lt, 40, (4, 9)),
        (Kind::Le, 30, (0, 4)),
        (Kind::Gt, 30, (5, 9)),
        (Kind::Ge, 30, (4, 9)),
        (Kind::Eq, 30, (4, 4)),
        (Kind::Ne, 40, (4, 4)),
        (Kind::Ne, 30, (0, 9)),
    ] {
        let mut block = guarded_loop().blocks[2].clone();
        let Arg::Held(held) = &block.ops[0].args[0] else {
            panic!("the comparison reads the counter");
        };
        let counter = held.value;
        block.ops[1].test = Some(test);
        let known = IndexMap::from_iter([(counter, interval(0, 9, 2))]);

        let result = on_edge(&block, successor, &known, None).unwrap().unwrap();

        assert_eq!(result[&counter], interval(low, high, 2), "{test:?} to {successor}");
    }
}

#[test]
fn test_non_comparison_flags_do_not_establish_a_bound() {
    // `tests/test_edge_ranges.py`, both parametrized cases.
    for (operation, kind) in [(Operation::Binary, Kind::Sub), (Operation::Compare, Kind::And)] {
        let mut block = guarded_loop().blocks[2].clone();
        let Arg::Held(held) = &block.ops[0].args[0] else {
            panic!("the comparison reads the counter");
        };
        let known = IndexMap::from_iter([(held.value, interval(0, 9, 2))]);
        block.ops[0].op = Some(OpCode::Operation(operation));
        block.ops[0].kind = kind;

        assert_eq!(on_edge(&block, 30, &known, None).unwrap(), Some(known));
    }
}

fn unary(kind: Kind, operation_: Operation, result_width: u32, extra: Option<Arg>) -> (Op, Value) {
    let (source, result) = (value(1, 0), value(2, 0));
    let mut made = operation(0, operation_, kind, vec![result], vec![source]);
    made.args = [Some(word(source)), extra].into_iter().flatten().collect();
    made.results = vec![Arg::Held(Held {
        value: result,
        width: result_width,
    })];
    (made, source)
}

#[test]
fn test_unit_steps_require_nonwrapping_intervals() {
    // `tests/test_ranges.py`, every parametrized case.
    for (kind, low, high, expected) in [
        (Kind::Decrement, 1, 3, Some(interval(0, 2, 2))),
        (Kind::Increment, -3, -1, Some(interval(-2, 0, 2))),
        (Kind::Decrement, -32768, 0, None),
        (Kind::Increment, 0, 32767, None),
    ] {
        let (made, source) = unary(kind, Operation::Unary, 2, None);
        let known = IndexMap::from_iter([(source, interval(low, high, 2))]);
        assert_eq!(_computed(&made, &known, &IndexMap::default()), expected);
    }
}

#[test]
fn test_signed_widening_keeps_the_numeric_range() {
    // `tests/test_ranges.py`: ADDRM's bounded 1..20 counter lost its interval when converted to a long.
    for (low, high) in [(1, 20), (-32768, -1), (-10, 10)] {
        let (made, source) = unary(Kind::SignExtend, Operation::Extend, 4, None);
        let known = IndexMap::from_iter([(source, interval(low, high, 2))]);
        assert_eq!(_computed(&made, &known, &IndexMap::default()), Some(interval(low, high, 4)));
    }
}

#[test]
fn test_secondary_recurrence_bounds_reject_wrap() {
    // `tests/test_ranges.py`, every parametrized case.
    for (start, step, advances, expected) in [
        (0, 4, 5, Some(interval(0, 20, 2))),
        (20, -4, 5, Some(interval(0, 20, 2))),
        (7, 0, 5, Some(interval(7, 7, 2))),
        (32760, 4, 1, None),
        (-32760, -4, 2, None),
        (0, 16384, 4, None),
        (0, 4, -1, None),
    ] {
        let found = _recurrence_span(&BigInt::from(start), &BigInt::from(step), &BigInt::from(advances), 2);
        assert_eq!(found, expected, "{start} {step} {advances}");
    }
}

#[test]
fn test_shift_ranges_refuse_wraparound() {
    // `tests/test_ranges.py`, every parametrized case.
    for (low, high, count, expected) in [
        (0, 5, 2, Some(interval(0, 20, 2))),
        (-5, -1, 2, Some(interval(-20, -4, 2))),
        (0, 16384, 1, None),
        (-32768, -1, 1, None),
        (0, 5, 32, None),
    ] {
        let (made, source) = unary(Kind::Shl, Operation::Binary, 2, Some(Arg::Const(Const::new(count, 1))));
        let known = IndexMap::from_iter([(source, interval(low, high, 2))]);
        assert_eq!(_computed(&made, &known, &IndexMap::default()), expected, "{low} {high} {count}");
    }
}

#[test]
fn test_range_alias_checks_cover_width_and_wrap() {
    for (known, offset, width, disjoint) in [
        (interval(0, 20, 2), 26, 2, true),
        (interval(0, 20, 2), 25, 2, false),
        (interval(0, 20, 2), 100, 4, true),
        (interval(-8, 20, 2), 100, 4, false),
        (interval(0, 65535, 2), 100, 4, false),
        (interval(0, 20, 4), 100, 4, false),
    ] {
        let base = value(1, 0);
        let mut address = Addr::new(Space::Segment, 4);
        address.index = 5;
        address.base = iced_x86::Register::SI;
        let mut indexed = MemRef::new(Some(address), 2);
        indexed.base = Some(base);
        indexed.base_width = 2;
        let mut address = Addr::new(Space::Segment, offset);
        address.index = 5;
        let fixed = MemRef::new(Some(address), width);
        let facts = std::collections::BTreeMap::from([(base, known)]);
        assert_eq!(overlapping(&indexed, &fixed, Some(&facts), None, None).unwrap(), !disjoint);
        assert!(overlapping(&indexed, &fixed, None, None, None).unwrap());
    }
}

#[test]
fn test_unsigned_edge_never_removes_a_possible_selector() {
    for kind in [Kind::Above, Kind::AboveEq, Kind::Below, Kind::BelowEq] {
        for span in [(1, 3), (255, 255), (256, 260), (-3, -1), (-2, 2)] {
            for successor in [30, 40] {
                for width in [2, 4] {
                    let mut block = guarded_loop().blocks[2].clone();
                    let Arg::Held(held) = block.ops[0].args[0] else {
                        panic!("the comparison reads the counter");
                    };
                    let counter = held.value;
                    block.ops[0].args = vec![Arg::Held(Held { value: counter, width }), Arg::Const(Const::new(255, width))];
                    block.ops[1].test = Some(kind);
                    let target = block.ops[1].target.unwrap();
                    let known = IndexMap::from_iter([(counter, interval(span.0, span.1, width))]);
                    let answer = |value: i64| match kind {
                        Kind::Above => value > 255,
                        Kind::AboveEq => value >= 255,
                        Kind::Below => value < 255,
                        _ => value <= 255,
                    };
                    let possible = (span.0..=span.1)
                        .any(|number| answer(number & ((1_i64 << (8 * width)) - 1)) == (successor == target));
                    let result = on_edge(&block, successor, &known, None).unwrap();
                    assert_eq!(result.is_some(), possible, "{kind:?} {span:?} {successor} {width}");
                }
            }
        }
    }
}
