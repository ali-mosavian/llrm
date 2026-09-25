//! Port of `tests/test_consts.py`.
//!
//! `test_constant_analysis_scope_reuses_an_unchanged_body_without_sharing_mutation`
//! keeps its mutation half; counting `_solved` calls needs a monkeypatch.

use std::rc::Rc;
use std::collections::BTreeSet;
use std::sync::Arc;

use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use super::{_defined, _result, ARITH, Cells, Known, UNARY, known, masked, reusing};
use crate::backend::lower::{self, Placed};
use crate::frontends::bc::blocks::Block;
use crate::model::ir::nodes::{span, Node};
use crate::model::ir::{self, Loc, Operation};
use crate::model::mir::{self, Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Synth, Value};
use crate::objectfile::module::tests::objects;
use crate::objectfile::module::{Addr, Space};
use crate::optimize::transform;
use crate::support::testing;

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
        let mut literal = widen.clone();
        literal.args = vec![Arg::Const(Const::new(number, 2))];
        literal.uses = vec![];
        let body = Rc::new(MirBody::new(0, vec![MirBlock::new(0, vec![], vec![literal], vec![])]));
        let folded = transform::folded(&body, &BTreeSet::new(), &IndexMap::default()).unwrap();
        assert_eq!(folded.blocks[0].ops[0].kind, Kind::Copy);
        assert_eq!(folded.blocks[0].ops[0].args, [Arg::Const(Const::new(expected, 4))]);
    }
}

/// NDMAX's zero displacement should fold without interpreting its base as an integer offset.
#[test]
fn test_pointer_displacement_constants_preserve_order_and_width() {
    for width in [2, 4] {
        let (pointer, displacement, result) = (Value::new(990, 0), Value::new(991, 0), Value::new(992, 0));
        let args = vec![held(pointer, 4), held(displacement, 4)];
        let mut ptr = op(0, OpCode::Operation(Operation::Nothing), "", vec![result], vec![pointer, displacement], Kind::PtrOffset);
        ptr.args = args.clone();
        ptr.results = vec![held(result, 4)];
        let changed = transform::_constant_operands(
            &ptr,
            &facts([(pointer, Known::new(0x1234_0000, 4)), (displacement, Known::new(0, width))]),
            None,
            None,
        );
        let second = if width == 4 { Arg::Const(Const::new(0, 4)) } else { args[1].clone() };
        assert_eq!(changed.args, [args[0].clone(), second], "{width}");
        assert!(changed.uses.contains(&pointer));
    }
}

/// Every body of `obj`, raised one at a time from its decoded IR.
fn raised(obj: &str) -> Vec<Rc<MirBody>> {
    let found = testing::loaded(obj).unwrap();
    let partitioned = testing::partitioned(obj);
    let Ok(result) = testing::bodies(obj) else {
        return vec![];
    };
    if partitioned.is_empty() {
        return vec![];
    }
    let nodes: IndexMap<i64, Arc<Node>> =
        result.iter().flat_map(|body| &body.nodes).map(|node| (span(node).0 as i64, node.clone())).collect();
    let mut out = vec![];
    for body in &result {
        let mine: Vec<Block> = partitioned
            .iter()
            .filter(|block| body.body.ranges.iter().any(|&(lo, hi)| lo <= block.at && block.at < hi))
            .cloned()
            .collect();
        if mine.is_empty() {
            continue;
        }
        let built = mir::raise_body(&mine, &nodes, Some(body.body.seed as i64), Some(&found.calls), None, None, None, None)
            .unwrap()
            .unwrap_or_else(|why| panic!("{obj}: {why}"));
        out.push(Rc::new(built.body));
    }
    out
}

fn fixtures() -> Vec<String> {
    objects().iter().map(|path| path.to_string_lossy().into_owned()).collect()
}

/// HARR's descriptor at segment 5 + 6 was reported as the constant zero.
#[test]
fn test_relocated_descriptor_address_is_not_integer_zero() {
    let obj = "tests/fixtures/omf/harr-p-g2.obj";
    let found = testing::loaded(obj).unwrap();
    assert_eq!(found.operands[&0x70].disp, 6);
    let body = raised(obj).into_iter().find(|body| testing::ops(body).iter().any(|op| op.at == 0x6F)).unwrap();
    let op = testing::ops(&body).into_iter().find(|op| op.at == 0x6F).unwrap();
    assert!(!known(&body, None, None, None, None).contains_key(&op.defines[0]));
    let Placed::Loc(Loc::Imm(immediate)) = lower::operand(&op.args[0]) else { panic!("{:?}", op.args[0]) };
    assert_eq!(immediate.address, Some(found.operands[&0x70]));
}

/// CHAIN printed MODMOD=92344 instead of 13106 after stale DX replaced a folded high word.
#[test]
fn test_folded_extraction_has_no_implicit_machine_result() {
    let found = testing::module("tests/fixtures/regressions/chain-stack-q-o.obj");
    let body = testing::main_body(&found, &testing::blocks_of(&found));
    let folded = transform::folded(&body, &found.dgroup.members, &found.calls).unwrap();
    let folded = transform::folded(&folded, &found.dgroup.members, &found.calls).unwrap();
    let extracts: BTreeSet<Value> = testing::ops(&body)
        .iter()
        .filter(|op| op.kind == Kind::Extract)
        .map(|op| match &op.results[0] {
            Arg::Held(one) => one.value,
            other => panic!("{other:?}"),
        })
        .collect();
    let copies: Vec<Op> = testing::ops(&folded)
        .into_iter()
        .filter(|op| {
            op.kind == Kind::Copy
                && matches!(op.results.first(), Some(Arg::Held(one)) if extracts.contains(&one.value))
                && matches!(op.args[0], Arg::Const(_))
        })
        .collect();
    assert!(!copies.is_empty());
    let calls = IndexMap::default();
    let lowering = lower::Lowering::new(
        &folded,
        extracts.iter().map(|value| value.id).collect(),
        &calls,
        BTreeSet::new(),
        None,
        "386",
        lower::Options::default(),
    )
    .unwrap();
    for op in &copies {
        assert_eq!(lowering._idiom(op, false).unwrap(), vec![]);
    }
}

/// NESTED kept constant loop bounds in registers; propagating them must not reverse subtraction.
#[test]
fn test_constant_subtraction_preserves_operand_order() {
    for constant_first in [false, true] {
        for same in [false, true] {
            let (mut source, bound, result) = (Value::new(910, 0), Value::new(911, 0), Value::new(912, 1));
            if same {
                source = bound;
            }
            let mut args = vec![held(source, 2), held(bound, 2)];
            if constant_first {
                args.reverse();
            }
            let mut uses = vec![source];
            if bound != source {
                uses.push(bound);
            }
            let mut compare = op(1, OpCode::Operation(Operation::Compare), "cmp", vec![result], uses, Kind::Sub);
            compare.args = args.clone();
            let changed = transform::_constant_operands(&compare, &facts([(bound, Known::new(5, 2))]), None, None);
            let expected =
                if !constant_first || same { vec![args[0].clone(), Arg::Const(Const::new(5, 2))] } else { args.clone() };
            assert_eq!(changed.args, expected, "{constant_first} {same}");
            let Arg::Held(first) = &args[0] else { unreachable!() };
            assert!(changed.uses.contains(&first.value));
        }
    }
}

/// Substituting p in memory[p] - p orphaned the address value while the load still used it.
#[test]
fn test_constant_operand_keeps_its_memory_address_dependency() {
    for address_part in ["base", "segment"] {
        let (pointer, result) = (Value::new(920, 0), Value::new(921, 1));
        let mut reference = MemRef::new(None, 2);
        if address_part == "base" {
            reference.base = Some(pointer);
        } else {
            reference.segment = Some(pointer);
        }
        let mut sub = op(1, OpCode::Operation(Operation::Binary), "sub", vec![result], vec![pointer], Kind::Sub);
        sub.args = vec![Arg::Cell(Cell { r#ref: reference.clone() }), held(pointer, 2)];
        sub.results = vec![held(result, 2)];
        sub.loads = vec![reference.clone()];
        let changed = transform::_constant_operands(&sub, &facts([(pointer, Known::new(16, 2))]), None, None);
        assert_eq!(changed.args, [Arg::Cell(Cell { r#ref: reference }), Arg::Const(Const::new(16, 2))]);
        assert_eq!(changed.uses, [pointer]);
    }
}

#[test]
fn test_a_known_factor_becomes_a_multiply_operand() {
    for constant_first in [false, true] {
        let (source, factor, result) = (Value::new(900, 0), Value::new(901, 0), Value::new(902, 1));
        let mut args = vec![held(source, 2), held(factor, 2)];
        if constant_first {
            args.reverse();
        }
        let mut multiply = op(1, OpCode::Operation(Operation::Multiply), "", vec![result], vec![source, factor], Kind::Mul);
        multiply.args = args;
        multiply.results = vec![held(result, 2)];
        let changed = transform::_constant_operands(&multiply, &facts([(factor, Known::new(20, 2))]), None, None);
        assert_eq!(changed.args, [held(source, 2), Arg::Const(Const::new(20, 2))]);
        assert_eq!(changed.uses, [source]);
    }
}

/// `mov ax,5` does not make eax five. Claiming it would fold a 32-bit use of
/// a value only half of which is known, and the answer would look reasonable.
#[test]
fn test_a_fact_never_claims_more_bytes_than_the_instruction_wrote() {
    for obj in fixtures() {
        for body in raised(&obj) {
            for fact in known(&body, None, None, None, None).values() {
                assert!([1, 2, 4].contains(&fact.width), "{obj}");
                assert!(BigInt::from(0) <= fact.n && fact.n < BigInt::from(1) << (fact.width * 8), "{obj}");
            }
        }
    }
}

#[test]
fn test_every_known_value_is_defined_by_an_operation_that_computes_it() {
    for obj in fixtures() {
        for body in raised(&obj) {
            let facts = known(&body, None, None, None, None);
            let defined: IndexMap<Option<Value>, Op> =
                testing::ops(&body).into_iter().map(|op| (_defined(&op), op)).collect();
            for value in facts.keys() {
                let op = defined.get(&Some(*value)).unwrap_or_else(|| panic!("{obj}: {value:?} is known but nothing defines it"));
                let node = op.node().unwrap_or_else(|| panic!("{obj}: {value:?}"));
                assert!(ir::modelled(node.semantics()), "{obj}: {value:?}");
            }
        }
    }
}

/// BC materialises a comparison as `mov ax,0` then a conditional `dec ax` --
/// and -1 is what BASIC calls true.
#[test]
fn test_a_comparison_result_folds_to_basics_own_true() {
    let mut seen = 0;
    for body in raised("tests/fixtures/omf/cmpord-p-evt.obj") {
        let facts = known(&body, None, None, None, None);
        for op in testing::ops(&body) {
            let Some(target) = _defined(&op) else { continue };
            if op.name == "dec" && facts.contains_key(&target) {
                assert_eq!(facts[&target], Known::new(0xFFFF, 2));
                seen += 1;
            }
        }
    }
    assert!(seen > 0, "cmpord materialises comparison results");
}

/// A phi is where two definitions meet, so its value is not one of them.
#[test]
fn test_nothing_is_folded_through_a_phi() {
    for obj in fixtures().into_iter().take(20) {
        for body in raised(&obj) {
            let facts = known(&body, None, None, None, None);
            for phi in body.blocks.iter().flat_map(|block| &block.phis) {
                assert!(!facts.contains_key(&phi.result), "{obj}");
            }
        }
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
