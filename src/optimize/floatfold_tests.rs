//! Port of tests/test_floatfold.py.

use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use super::*;
use crate::analysis::floatfacts::{Finite, Fraction};
use crate::model::floating::{Precision, Rounding, Semantics};
use crate::model::ir::{Addr, Operation, Space};
use crate::model::mir::{Held, MemRef, MirBlock, OpCode};
use crate::support::testing;

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
        let facts = IndexMap::from_iter([(source, Finite::new(number, false))]);
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
        let mut converted = IndexMap::from_iter([(result, Known::new(144, 4))]);
        let mut extra = vec![];
        match guard {
            "unknown" => converted = IndexMap::default(),
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
        assert!(changed.blocks[0].ops.iter().all(|op| {
            let lowered = crate::backend::lower::semantics(op, None, crate::backend::lower::Place::Default);
            lowered.unwrap().unwrap().name.as_deref() == Some("wait")
        }));
        assert_eq!(crate::optimize::transform::dead(&changed).unwrap(), changed);
    }
}

/// FPCSE's WAIT was called NOTHING in MIR, hiding an observation boundary from passes.
#[test]
#[ignore = "fails in Python too: AttributeError: 'Op' object has no attribute 'node'"]
fn test_original_wait_is_an_explicit_checkpoint_with_encoding_provenance() {
    let body = testing::nth(&testing::raised_with("tests/fixtures/omf/fpcse-p-g2.obj", true, false), 0);
    let check = testing::ops(&body).into_iter().find(|op| op.at == 0x7A).unwrap();
    assert_eq!(check.kind, Kind::Fcheck);
    assert!(check.name.is_empty() && check.node().is_some());
    assert_eq!(check.raising.as_ref().and_then(|raising| raising.covers), Some((0x7A, 0x7C)));
    let what = crate::backend::lower::current(&check, crate::backend::lower::Place::Default, None).unwrap().unwrap();
    assert_eq!(what.name.as_deref(), Some("wait"));
}

fn emitted(path: &str) -> Vec<iced_x86::Instruction> {
    testing::instructions(&testing::emitted_lir(path).data)
}

/// QB FPDEEP still computed d=12 and e=6 on x87 after proving both exact.
#[test]
fn test_fpdeep_exact_double_stores_do_not_execute_floating_arithmetic() {
    use iced_x86::Mnemonic;
    let instructions = emitted("tests/fixtures/omf/fpdeep-q-o.obj");
    let arithmetic = [Mnemonic::Fld, Mnemonic::Fmul, Mnemonic::Fmulp, Mnemonic::Fdiv, Mnemonic::Fdivp];
    assert!(!instructions.iter().any(|one| arithmetic.contains(&one.mnemonic())));
    assert!(!instructions.iter().any(|one| one.mnemonic() == Mnemonic::Wait));
}

/// QB FPCSE falsely reported five overlapping bytes when entry 0x30 became source 0x35.
#[test]
fn test_qb_fpcse_preserves_entry_when_first_load_disappears() {
    testing::emitted_lir("tests/fixtures/omf/fpcse-q-o.obj");
}

/// QB FPCSE's constant result still ran three jumps through its empty loop header.
#[test]
fn test_collapsed_fpcse_has_no_empty_jump_trampoline() {
    assert!(!emitted("tests/fixtures/omf/fpcse-q-o.obj").iter().any(|one| one.mnemonic() == iced_x86::Mnemonic::Jmp));
}
