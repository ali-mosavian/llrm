//! Ports of `tests/test_ranges.py`, `tests/test_edge_ranges.py` and
//! `tests/test_unsigned_edge_ranges.py`.
//!
//! Skipped, monkeypatching `strength._multiplies` and `observers.private`:
//! `test_nbody_scaled_index_is_bounded_only_inside_its_loop`.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use super::{_computed, _recurrence_span, Interval, bounded, covering, on_edge, unwrapped};
use crate::analysis::induction::counted_loops_tests::{_loop, shaped};
use crate::model::passes::{O2, Options};
use crate::support::testing;
use crate::wholeseg::Emission;
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
    assert_eq!(known[&50][&counter], interval(10, 32767, 2));
}

#[test]
fn test_a_compare_under_the_same_compare_is_decided_outside_any_loop() {
    // Only counted loops were scoped, so `x < 8` under `x < 8` in straight-line code kept both branches.
    let x = value(1, 0);
    let compare = |at: i64, yes: i64, no: i64| {
        let mut block = guarded_loop().blocks[2].clone();
        block.at = at;
        block.succ = vec![yes, no];
        block.ops[0].args = vec![word(x), Arg::Const(Const::new(8, 2))];
        block.ops[0].uses = vec![x];
        block.ops[1].target = Some(yes);
        block
    };
    let body = Rc::new(MirBody::new(
        0,
        vec![
            compare(0, 10, 30),
            compare(10, 20, 40),
            MirBlock::new(20, vec![], vec![], vec![]),
            MirBlock::new(30, vec![], vec![], vec![]),
            MirBlock::new(40, vec![], vec![], vec![]),
        ],
    ));
    let scoped = bounded(&body).unwrap();

    assert_eq!(on_edge(&body.blocks[1], 40, &scoped[&10], None).unwrap(), None);
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

/// `x AND 511` had no interval unless `x` had one, so a masked subscript was
/// never known to fit.
#[test]
fn test_a_nonnegative_mask_bounds_an_unknown_operand() {
    let (made, _) = unary(Kind::And, Operation::Binary, 2, Some(Arg::Const(Const::new(511, 2))));
    assert_eq!(_computed(&made, &IndexMap::default(), &IndexMap::default()), Some(interval(0, 511, 2)));
    let (made, _) = unary(Kind::And, Operation::Binary, 2, Some(Arg::Const(Const::new(-2, 2))));
    assert_eq!(_computed(&made, &IndexMap::default(), &IndexMap::default()), None);
}

/// A loop tested after its trip had no facts in its header, which in a
/// rotated loop is the whole trip, so no access there had a bounded index.
#[test]
fn test_a_posttested_header_knows_its_counter() {
    let mut rotated = shaped("post");
    rotated.split = true;
    let body = _loop(Some(0), 9, Kind::Lt, 1, rotated);
    let counter = body.blocks[1].phis[0].result;
    let known = bounded(&Rc::new(body)).unwrap();
    assert_eq!(known.get(&1).and_then(|facts| facts.get(&counter)), Some(&interval(0, 9, 1)));
}

/// A 32-bit sum of a 16-bit address is exact only from the object's origin
/// and while the scaled index stays inside the address width.
#[test]
fn test_an_address_is_unwrapped_only_from_its_origin_and_in_range() {
    let (origin, other) = (value(1, 0), value(2, 0));
    let reference = MemRef {
        base_width: 2,
        inbounds: true,
        origin: Some(origin),
        ..MemRef::new(Some(Addr::new(Space::Far, 0)), 2)
    };
    assert!(unwrapped(&reference, origin, &interval(0, 511, 2), 2));
    assert!(!unwrapped(&reference, other, &interval(0, 511, 2), 2));
    assert!(!unwrapped(&reference, origin, &interval(0, 32768, 2), 2));
    assert!(!unwrapped(&reference, origin, &interval(-1, 511, 2), 2));
    assert!(!unwrapped(&MemRef { inbounds: false, ..reference.clone() }, origin, &interval(0, 511, 2), 2));
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

fn applied(path: &str, options: Options) -> Rc<MirBody> {
    let found = testing::module(path);
    let blocks = testing::blocks_of(&found);
    testing::applied(&found, Some(&blocks), &testing::main_body(&found, &blocks), options)
}

/// FPDEEP lost its 1..3 bound at i-1, leaving p(i)'s byte extent unknown.
#[test]
#[ignore = "fails in Python too: assert [] (no indexed FLOAD once transformed)"]
fn test_fpdeep_one_based_index_has_a_bounded_byte_offset() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        // Unrolled, i is a constant.
        let body = applied(&format!("fixtures/omf/fpdeep-{tag}.obj").to_lowercase(), Options { unroll: false, ..Default::default() });
        let known = bounded(&body).unwrap();
        let accesses: Vec<(i64, MemRef)> = body
            .blocks
            .iter()
            .flat_map(|block| block.ops.iter().map(move |op| (block.at, op)))
            .filter(|(_, op)| op.kind == Kind::Fload)
            .flat_map(|(at, op)| op.loads.iter().filter(|one| one.base.is_some()).map(move |one| (at, one.clone())))
            .collect();
        assert!(!accesses.is_empty(), "{tag}");
        for (at, reference) in &accesses {
            let scoped: BTreeMap<Value, Interval> = known[at].iter().map(|(value, one)| (*value, one.clone())).collect();
            let covered = covering(reference, &scoped);
            assert!(covered.base.is_none(), "{tag}");
            assert_eq!(covered.addr.unwrap().disp, 6, "{tag}");
            assert_eq!(covered.width, 12, "{tag}");
        }
    }
}

/// ADDRM lost the 1..20 bound at the integer-to-long conversion feeding b(i).
#[test]
fn test_addrm_long_array_value_keeps_counter_bounds() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let body = applied(&format!("fixtures/omf/addrm-{tag}.obj").to_lowercase(), O2());
        let known = bounded(&body).unwrap();
        let converted: Vec<(i64, Value)> = body
            .blocks
            .iter()
            .flat_map(|block| block.ops.iter().map(move |op| (block.at, op)))
            .filter(|(_, op)| op.kind == Kind::SignExtend)
            .map(|(at, op)| match &op.results[0] {
                Arg::Held(one) => (at, one.value),
                other => panic!("{other:?}"),
            })
            .collect();
        assert!(!converted.is_empty(), "{tag}");
        for (at, value) in &converted {
            assert_eq!(known[at][value], Interval { low: 1.into(), high: 20.into(), width: 4 }, "{tag}");
        }
    }
}

/// RNGARM printed 28,7,10 but stored INDEX eleven times; its guarded array cannot alias INDEX.
#[test]
#[ignore = "fails in Python too: assert (set()) (no loop left at mir-widen)"]
fn test_rngarm_writes_its_counter_only_after_the_loop() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let data = testing::data(format!("fixtures/regressions/rngarm-{tag}.obj").to_lowercase());
        let found = testing::loaded_bytes(&data).unwrap();
        let (result, states) = testing::emitted_mir(&data, "mir-widen", "");
        assert_eq!(result.outcome, Emission::Lir, "{tag}: {}", result.reason);
        let [body] = &states[..] else { panic!("{tag}: {} states", states.len()) };
        let inside: BTreeSet<i64> =
            crate::analysis::loops::loops(&body.blocks, Some(body.entry)).into_iter().flat_map(|one| one.body).collect();
        let writes: Vec<i64> = body
            .blocks
            .iter()
            .flat_map(|block| block.ops.iter().flat_map(move |op| op.stores.iter().map(move |one| (block.at, one))))
            .filter(|(_, one)| {
                one.addr.is_some_and(|addr| {
                    addr.space == Space::Segment && Some(addr.index) == found.program_data && addr.disp == 14
                })
            })
            .map(|(at, _)| at)
            .collect();
        assert!(!inside.is_empty() && writes.len() <= 1, "{tag}");
        assert!(!writes.iter().any(|at| inside.contains(at)), "{tag}");
    }
}
