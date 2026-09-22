//! Port of `tests/test_consts.py`.
//!
//! Skipped, needing `transform`, `lower` or the corpus raise:
//! `test_pointer_displacement_constants_preserve_order_and_width`,
//! `test_relocated_descriptor_address_is_not_integer_zero`,
//! `test_folded_extraction_has_no_implicit_machine_result`,
//! `test_constant_subtraction_preserves_operand_order`,
//! `test_constant_operand_keeps_its_memory_address_dependency`,
//! `test_a_known_factor_becomes_a_multiply_operand`,
//! `test_a_fact_never_claims_more_bytes_than_the_instruction_wrote`,
//! `test_every_known_value_is_defined_by_an_operation_that_computes_it`,
//! `test_a_comparison_result_folds_to_basics_own_true`,
//! `test_nothing_is_folded_through_a_phi`.
//! `test_signed_widening_produces_a_whole_long_constant` keeps its
//! `_result` half; the `transform.folded` half is skipped.
//! `test_constant_analysis_scope_reuses_an_unchanged_body_without_sharing_mutation`
//! keeps its mutation half; counting `_solved` calls needs a monkeypatch.

use std::rc::Rc;
use std::collections::BTreeSet;

use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use super::{_defined, _result, ARITH, Cells, Known, UNARY, known, masked, reusing};
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Synth, Value};
use crate::objectfile::module::{Addr, Space};

fn op(at: i64, operation: OpCode, name: &str, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
    let mut made = Op::new(at, operation, name, defines, uses);
    made.kind = kind;
    made
}

fn held(value: Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

fn facts<const N: usize>(items: [(Value, Known); N]) -> IndexMap<Value, Known> {
    IndexMap::from_iter(items)
}

#[test]
fn test_an_index_constant_is_not_the_value_of_an_indexed_store() {
    let (index, source, loaded) = (Value::new(1, 0), Value::new(2, 0), Value::new(3, 0));
    let mut address = Addr::new(Space::Segment, 0x5A);
    address.index = 5;
    let mut reference = MemRef::new(Some(address), 2);
    reference.base = Some(index);
    reference.base_width = 2;
    let mut set_index = op(0, OpCode::Operation(Operation::Move), "mov", vec![index], vec![], Kind::Copy);
    set_index.args = vec![Arg::Const(Const::new(4, 2))];
    set_index.results = vec![held(index, 2)];
    let mut store = op(1, OpCode::Operation(Operation::Move), "mov", vec![], vec![source, index], Kind::Store);
    store.args = vec![held(source, 2)];
    store.results = vec![Arg::Cell(Cell { r#ref: reference.clone() })];
    store.stores = vec![reference.clone()];
    let mut load = op(2, OpCode::Operation(Operation::Move), "mov", vec![loaded], vec![index], Kind::Load);
    load.args = vec![Arg::Cell(Cell { r#ref: reference.clone() })];
    load.results = vec![held(loaded, 2)];
    load.loads = vec![reference];
    let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![set_index, store, load], vec![])]);

    let found = known(&Rc::new(MirBody::clone(&body)), Some(&BTreeSet::from([5])), Some(&IndexMap::default()), None, None);
    assert!(!found.contains_key(&loaded));
}

#[test]
fn test_constant_analysis_scope_reuses_an_unchanged_body_without_sharing_mutation() {
    let value = Value::new(1, 0);
    let mut copy = op(0, OpCode::Operation(Operation::Move), "", vec![value], vec![], Kind::Copy);
    copy.args = vec![Arg::Const(Const::new(7, 2))];
    copy.results = vec![held(value, 2)];
    let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![copy], vec![])]);
    let second = reusing(|| {
        let mut first = known(&Rc::new(MirBody::clone(&body)), None, None, None, None);
        first.insert(value, Known::new(99, 2));
        known(&Rc::new(MirBody::clone(&body)), None, None, None, None)
    });
    assert_eq!(second[&value], Known::new(7, 2));
}

#[test]
fn test_signed_widening_produces_a_whole_long_constant() {
    for number in [0_i64, 1, 32767, 32768, 65535] {
        let (source, result) = (Value::new(1, 0), Value::new(2, 0));
        let mut widen = op(0, OpCode::Operation(Operation::Extend), "", vec![result], vec![source], Kind::SignExtend);
        widen.args = vec![held(source, 2)];
        widen.results = vec![held(result, 4)];
        let expected = ((number ^ 0x8000) - 0x8000) & 0xFFFF_FFFF;
        assert_eq!(
            _result(&widen, &facts([(source, Known::new(number, 2))]), None, None),
            Some(Known::new(expected, 4))
        );
        assert_eq!(_result(&widen, &facts([(source, Known::new(number, 1))]), None, None), None);
    }
}

#[test]
fn test_word_extension_to_int64_preserves_signedness() {
    for (kind, number, expected) in [
        (Kind::SignExtend, 0x8001_u64, 0xFFFF_FFFF_FFFF_8001_u64),
        (Kind::ZeroExtend, 0x8001, 0x8001),
    ] {
        let (source, result) = (Value::new(1, 0), Value::new(2, 0));
        let mut widen = op(0, OpCode::Operation(Operation::Nothing), "", vec![result], vec![source], kind);
        widen.args = vec![held(source, 2)];
        widen.results = vec![held(result, 8)];
        assert_eq!(
            _result(&widen, &facts([(source, Known::new(number, 2))]), None, None),
            Some(Known::new(expected, 8))
        );
    }
}

#[test]
fn test_extension_of_a_known_memory_cell_folds_to_the_extended_value() {
    let result = Value::new(1, 0);
    let mut address = Addr::new(Space::Segment, 0);
    address.index = 7;
    let reference = MemRef::new(Some(address), 1);
    let mut widen = op(0, OpCode::Operation(Operation::Extend), "", vec![result], vec![], Kind::ZeroExtend);
    widen.args = vec![Arg::Cell(Cell { r#ref: reference.clone() })];
    widen.results = vec![held(result, 4)];
    widen.loads = vec![reference];
    let here = Cells::from_iter([((address, 1), Known::new(0xF1, 1))]);
    assert_eq!(_result(&widen, &IndexMap::default(), Some(&here), None), Some(Known::new(0xF1, 4)));
}

#[test]
fn test_recovered_argument_constants() {
    for (high, low, answer) in [(4_i64, 0_i64, 262_144_i64), (0, 512, 512), (-1, -1, 0xFFFF_FFFF)] {
        let (upper, bottom, result) = (Value::new(1, 0), Value::new(2, 0), Value::new(3, 0));
        let mut concat = op(0, OpCode::Synth(Synth::ConcatLow), "concat", vec![result], vec![upper, bottom], Kind::Concat);
        concat.args = vec![held(upper, 2), held(bottom, 2)];
        concat.results = vec![held(result, 4)];
        let mut known = facts([(upper, Known::new(high, 2)), (bottom, Known::new(low, 2))]);
        assert_eq!(_result(&concat, &known, None, None), Some(Known::new(answer, 4)));
        known.insert(upper, Known::new(high, 1));
        assert_eq!(_result(&concat, &known, None, None), None);
    }
}

#[test]
fn test_a_fact_is_masked_to_its_own_width() {
    for (n, width, want) in [(5_i64, 2, 5_i64), (-1, 2, 0xFFFF), (0x1FFFF, 2, 0xFFFF), (-1, 4, 0xFFFF_FFFF)] {
        assert_eq!(masked(&BigInt::from(n), width), BigInt::from(want));
    }
}

#[test]
fn test_equal_integer_operands_are_zero_without_input_facts() {
    for kind in [Kind::Xor, Kind::Sub] {
        for width in [1, 2, 4] {
            let (source, result) = (Value::new(900, 0), Value::new(901, 1));
            let mut cancel = op(1, OpCode::Operation(Operation::Binary), "", vec![result], vec![source], kind);
            cancel.args = vec![held(source, width), held(source, width)];
            cancel.results = vec![held(result, width)];
            assert_eq!(_result(&cancel, &IndexMap::default(), None, None), Some(Known::new(0, width)));
        }
    }
}

#[test]
fn test_a_narrow_shift_cannot_pull_bits_from_outside_its_operand() {
    for (width, number, answer) in [(1, 0x101_i64, 0_i64), (2, 0x1235_0000, 0), (2, 0x1235_8000, 0x4000)] {
        let (source, result) = (Value::new(1, 0), Value::new(2, 1));
        let mut shift = op(1, OpCode::Operation(Operation::Binary), "shr", vec![result], vec![source], Kind::Shr);
        shift.args = vec![held(source, width), Arg::Const(Const::new(1, width))];
        shift.results = vec![held(result, width)];
        assert_eq!(
            _result(&shift, &facts([(source, Known::new(number, 4))]), None, None),
            Some(Known::new(answer, width))
        );
    }
}

#[test]
fn test_int64_shift_uses_all_six_count_bits() {
    let (source, result) = (Value::new(1, 0), Value::new(2, 0));
    let mut shift = op(1, OpCode::Operation(Operation::Nothing), "", vec![result], vec![source], Kind::Shr);
    shift.args = vec![held(source, 8), Arg::Const(Const::new(36, 1))];
    shift.results = vec![held(result, 8)];
    let number = 0xFEDC_BA98_7654_3210_u64;
    assert_eq!(
        _result(&shift, &facts([(source, Known::new(number, 8))]), None, None),
        Some(Known::new(number >> 36, 8))
    );
}

#[test]
fn test_flags_do_not_stop_an_operation_being_folded() {
    let result = Value::new(1, 0);
    let flags = Value {
        flags: true,
        ..Value::new(2, 0)
    };
    let unary = OpCode::Operation(Operation::Unary);
    assert_eq!(_defined(&Op::new(0, unary, "dec", vec![flags, result], vec![])), Some(result));
    assert_eq!(_defined(&Op::new(0, unary, "dec", vec![flags], vec![])), None);
}

#[test]
fn test_constant_steps_wrap_at_the_value_width() {
    for width in [1, 2, 4] {
        for (kind, number, answer) in [(Kind::Increment, -1_i64, 0_i64), (Kind::Decrement, 0, -1)] {
            let result = Value::new(1, 0);
            let mut step = op(0, OpCode::Operation(Operation::Unary), "", vec![result], vec![], kind);
            step.args = vec![Arg::Const(Const::new(number, width))];
            step.results = vec![held(result, width)];
            assert_eq!(
                _result(&step, &IndexMap::default(), None, None),
                Some(Known::new(masked(&BigInt::from(answer), width), width))
            );
        }
    }
}

#[test]
fn test_division_is_not_folded() {
    for kind in [Kind::Div, Kind::Divmod, Kind::Udivmod] {
        assert!(!ARITH.iter().any(|(one, _)| *one == kind));
    }
    assert!(!ARITH.iter().any(|(one, _)| UNARY.iter().any(|(other, _)| other == one)));
}

#[test]
fn test_a_fact_is_never_wider_than_the_operation_that_made_it() {
    assert_eq!(masked(&BigInt::from(0x1FFFF), 2), BigInt::from(0xFFFF));
    assert_eq!(Known::new(5, 2).width, 2);
}

// ---- tests/test_constant_cells.py ----

#[test]
fn test_a_call_reaching_nonlocal_keeps_an_uncaptured_static_constant() {
    // A constant cell's key had no object, so it met every call's reach and died at each one.
    use crate::model::memory::{Identity, MemoryKind, MemoryObject, Provenance};

    let address = Addr { index: 5, ..Addr::new(Space::Segment, 6) };
    let r#static = MemoryObject {
        identity: Some(Identity::Tuple(vec![Identity::Space(Space::Segment), Identity::Int(5)])),
        captured: false,
        ..MemoryObject::new(MemoryKind::Global)
    };
    let mut reference = MemRef::new(Some(address), 2);
    reference.provenance = Some(Provenance::one_with_slice(r#static, 6, 8, 1, 1, BTreeSet::new()).unwrap());
    let mut reach = MemRef::new(None, 0);
    reach.provenance = Some(Provenance::one(MemoryObject::new(MemoryKind::Nonlocal)));
    let mut store = op(0, OpCode::Operation(Operation::Move), "mov", vec![], vec![], Kind::Store);
    store.args = vec![Arg::Const(Const::new(7, 2))];
    store.stores = vec![reference.clone()];
    let mut call = op(1, OpCode::Operation(Operation::Call), "", vec![], vec![], Kind::Call);
    call.stores = vec![reach];
    call.memory_complete = true;
    let mut read = op(2, OpCode::Operation(Operation::Move), "mov", vec![], vec![], Kind::Load);
    read.loads = vec![reference.clone()];
    let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![store, call, read], vec![])]);

    let before = super::cells(&body, &BTreeSet::from([5]), &IndexMap::default(), None, None, None, None, None);

    assert_eq!(super::_cell(&before[&(0, 2)], &reference), Some(Known::new(7, 2)));
}

/// `floatfacts.repeated` stores through a fresh clone each iteration; keyed by
/// address, a later reference reused an earlier one's answer, and FPCSE's QB
/// loop proved no exit, or the wrong 487.5.
#[test]
fn test_a_reference_at_a_reused_address_is_resolved_anew() {
    let mut queries = super::_MemoryQueries::new(&IndexMap::default(), &BTreeSet::new());
    let first = MemRef::new(Some(Addr::new(Space::Segment, 0x12)), 4);
    let second = MemRef::new(Some(Addr::new(Space::Segment, 0x1a)), 4);
    let mut slot = first.clone();
    assert_eq!(*queries.resolve(&slot), first);
    slot = second.clone();
    assert_eq!(*queries.resolve(&slot), second);
}
