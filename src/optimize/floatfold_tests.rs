//! Port of tests/test_floatfold.py.
//!
//! Skipped until their modules are ported: `test_original_wait_is_an_explicit_checkpoint_with_encoding_provenance`
//! (corpus, `mir.bodies`), `test_fpdeep_exact_double_stores_do_not_execute_floating_arithmetic`,
//! `test_qb_fpcse_preserves_entry_when_first_load_disappears` and
//! `test_collapsed_fpcse_has_no_empty_jump_trampoline` (wholeseg).
//! `test_exact_pair_keeps_checks_and_refuses_observable_results` omits its
//! `lower.semantics` and `transform.dead` asserts (transform unported).

use indexmap::IndexMap;
use num_bigint::BigInt;

use super::*;
use crate::analysis::floatfacts::{Finite, Fraction};
use crate::model::floating::{Precision, Rounding, Semantics};
use crate::model::ir::{Addr, Operation, Space};
use crate::model::mir::{Held, MemRef, MirBlock, OpCode};

fn op(at: i64, operation: Operation, name: &str, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
    let mut op = Op::new(at, OpCode::Operation(operation), name, defines, uses);
    op.kind = kind;
    op
}

fn at(op: &Op, at: i64) -> Op {
    Op { at, ..op.clone() }
}

fn body(entry: i64, blocks: Vec<MirBlock>) -> MirBody {
    MirBody::new(entry, blocks)
}

#[test]
fn test_repeated_checkpoint_needs_no_new_floating_effect() {
    for interruption in [Kind::Copy, Kind::Store, Kind::Call, Kind::Opaque, Kind::Fload] {
        let check = op(0, Operation::Nothing, "", vec![], vec![], Kind::Fcheck);
        let middle = op(1, Operation::Move, "", vec![], vec![], interruption);
        let again = at(&check, 2);
        let body = body(0, vec![MirBlock::new(0, vec![], vec![check.clone(), middle, again], vec![])]);
        let changed = checks(&body);
        assert_eq!(changed.blocks[0].ops[0], check);
        assert_eq!(
            changed.blocks[0].ops.last().unwrap().kind,
            if matches!(interruption, Kind::Copy | Kind::Store) {
                Kind::Nothing
            } else {
                Kind::Fcheck
            }
        );
    }
}

#[test]
fn test_completed_fp_observation_crosses_only_proven_edges() {
    for interruption in [Kind::Copy, Kind::Call, Kind::Fload] {
        let check = op(0, Operation::Nothing, "", vec![], vec![], Kind::Fcheck);
        let middle = Op {
            at: 1,
            kind: interruption,
            ..check.clone()
        };
        let body = body(
            0,
            vec![
                MirBlock::new(0, vec![], vec![check.clone()], vec![1, 2]),
                MirBlock::new(1, vec![], vec![middle], vec![3]),
                MirBlock::new(2, vec![], vec![], vec![3]),
                MirBlock::new(3, vec![], vec![at(&check, 3)], vec![]),
            ],
        );
        let result = checks(&body);
        assert_eq!(result.blocks[0].ops[0].kind, Kind::Fcheck);
        assert_eq!(
            result.blocks.last().unwrap().ops[0].kind,
            if interruption == Kind::Copy {
                Kind::Nothing
            } else {
                Kind::Fcheck
            }
        );
    }
}

#[test]
fn test_loop_cannot_prove_its_first_fp_observation_redundant() {
    let check = op(1, Operation::Nothing, "", vec![], vec![], Kind::Fcheck);
    let body = body(
        0,
        vec![
            MirBlock::new(0, vec![], vec![], vec![1]),
            MirBlock::new(1, vec![], vec![check], vec![1]),
        ],
    );
    assert_eq!(checks(&body), body);
}

fn segment(disp: i64, index: i64) -> Addr {
    Addr {
        index,
        ..Addr::new(Space::Segment, disp)
    }
}

#[test]
fn test_storage_requires_exact_bits() {
    let third = Fraction::new(BigInt::from(1), BigInt::from(3));
    let integer = |n: i128| Fraction::from_integer(BigInt::from(n));
    let cases: Vec<(Format, u32, Fraction, Option<u64>)> = vec![
        (Format::Binary32, 4, integer(144), Some(0x43100000)),
        (Format::Binary32, 4, integer(-6), Some(0xC0C00000)),
        (Format::Binary32, 4, integer(16777217), None),
        (Format::Binary32, 4, third.clone(), None),
        (Format::Binary64, 8, integer(12), Some(0x4028000000000000)),
        (Format::Binary64, 8, integer(-6), Some(0xC018000000000000)),
        (Format::Binary64, 8, integer(9007199254740993), None),
        (Format::Binary64, 8, third, None),
    ];
    for (format, width, number, expected) in cases {
        let source = Value::new(1, 0);
        let cell = MemRef::new(Some(segment(0, 5)), width);
        let mut load = op(0, Operation::FloatLoad, "fld", vec![source], vec![], Kind::Fload);
        load.args = vec![Arg::Cell(Cell {
            r#ref: MemRef { width: 8, ..cell.clone() },
        })];
        load.results = vec![Arg::Held(Held { value: source, width: 10 })];
        load.floating = Some(Semantics::new([Format::Binary64], Format::Extended80, Precision::Exact, Rounding::None));
        let mut store = op(4, Operation::FloatStore, "fstp", vec![], vec![source], Kind::Fstore);
        store.args = vec![Arg::Held(Held { value: source, width: 10 })];
        store.results = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
        store.stores = vec![cell.clone()];
        store.floating = Some(Semantics::new([Format::Extended80], format, Precision::Destination, Rounding::Dynamic));
        let body = body(0, vec![MirBlock::new(0, vec![], vec![load, store], vec![])]);
        let facts = IndexMap::from([(source, Finite::new(number, false))]);
        let changed = stored(&Rc::new(MirBody::clone(&body)), &facts);
        let Some(expected) = expected else {
            assert_eq!(changed, Rc::new(body));
            continue;
        };
        assert_eq!(
            changed.blocks[0].ops.iter().map(|op| op.kind).collect::<Vec<_>>(),
            [Kind::Fcheck, Kind::Fcheck, Kind::Store]
        );
        let result = changed.blocks[0].ops.last().unwrap();
        assert_eq!(result.args, [Arg::Const(Const::new(expected, width))]);
        assert!(result.stores == [cell.clone()] && result.results == [Arg::Cell(Cell { r#ref: cell })]);
    }
}

#[test]
fn test_exact_pair_keeps_checks_and_refuses_observable_results() {
    for guard in ["none", "live", "shared", "memory", "unknown", "barrier"] {
        let (source, result) = (Value::new(1, 0), Value::new(2, 1));
        let cell = MemRef::new(Some(segment(0, 5)), 4);
        let mut load = op(0, Operation::FloatLoad, "fld", vec![source], vec![], Kind::Fload);
        load.args = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
        load.results = vec![Arg::Held(Held { value: source, width: 10 })];
        load.loads = vec![cell.clone()];
        load.floating = Some(Semantics::new([Format::Binary32], Format::Extended80, Precision::Exact, Rounding::None));
        let mut conversion = op(4, Operation::FloatStore, "fistp", vec![result], vec![source], Kind::Fstore);
        conversion.args = vec![Arg::Held(Held { value: source, width: 10 })];
        conversion.results = vec![Arg::Held(Held { value: result, width: 4 })];
        conversion.floating = Some(Semantics::new(
            [Format::Extended80],
            Format::Signed32,
            Precision::Destination,
            Rounding::Dynamic,
        ));
        let mut converted = IndexMap::from([(result, Known::new(144, 4))]);
        let mut extra = vec![];
        match guard {
            "unknown" => converted = IndexMap::new(),
            "memory" => conversion.stores = vec![cell.clone()],
            "barrier" => conversion.op = Some(OpCode::Operation(Operation::Barrier)),
            "live" | "shared" => {
                let value = if guard == "live" { result } else { source };
                extra.push(op(8, Operation::Call, "call", vec![], vec![value], Kind::Call));
            }
            _ => {}
        }
        let mut ops = vec![load, conversion];
        ops.extend(extra);
        let body = body(0, vec![MirBlock::new(0, vec![], ops, vec![])]);
        let changed = discarded(&Rc::new(MirBody::clone(&body)), &converted);
        if guard != "none" {
            assert_eq!(changed, Rc::new(body), "{guard}");
            continue;
        }
        assert_eq!(
            changed.blocks[0].ops.iter().map(|op| op.kind).collect::<Vec<_>>(),
            [Kind::Fcheck, Kind::Fcheck]
        );
    }
}
