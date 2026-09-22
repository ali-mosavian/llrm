//! Port of `tests/test_promote.py`.
//!
//! Skipped, needing modules not yet ported (OMF fixtures, `mir.bodies`,
//! `wholeseg`, `transform`, `loopmotion`):
//! test_guarded_indexed_accumulators_do_not_reload_in_loop,
//! test_procedure_frame_fields_reuse_stored_values,
//! test_unpromotable_memory_update_does_not_cancel_other_cells,
//! test_nested_memory_update_becomes_a_value_and_preserves_its_store,
//! test_promotion_preserves_existing_cse_value_edges,
//! test_hotlop_keeps_initialization_for_memory_arithmetic,
//! test_hotlop_multiply_uses_the_initialized_value,
//! test_a_read_before_assignment_keeps_its_memory_value,
//! test_promoted_global_remains_visible_outside_the_body,
//! test_production_press_keeps_the_loop_counter_in_a_value,
//! test_only_an_intervening_call_invalidates_a_stored_value,
//! test_spill_accumulator_is_a_loop_carried_value,
//! test_addrm_long_accumulator_survives_split_initialization.

use std::collections::BTreeSet;

use num_bigint::BigInt;

use super::*;
use crate::analysis::ranges::Interval;
use crate::model::ir::Operation;
use crate::model::memory::{Identity, MemoryKind, MemoryObject, Provenance};
use crate::model::mir::{
    Arg, ArrayRequest, Cell, Const, FrameAddress, Held, Kind, MemRef, MirBlock, MirBody, Op,
    OpCode, OrderedMap, Symbol, Value,
};
use crate::model::passes::{MIRTransform, Where};
use crate::objectfile::module::{Addr, Space};

fn value(id: u32, at: i64) -> Value {
    Value {
        variable: id,
        version: 1,
        ..Value::new(id, at)
    }
}

fn held(value: Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

fn constant(n: impl Into<BigInt>, width: u32) -> Arg {
    Arg::Const(Const::new(n, width))
}

fn cell(r#ref: &MemRef) -> Arg {
    Arg::Cell(Cell {
        r#ref: r#ref.clone(),
    })
}

fn addr(space: Space, disp: i64, index: i64) -> Addr {
    Addr {
        index,
        ..Addr::new(space, disp)
    }
}

fn object(kind: MemoryKind, identity: Identity, extent: i64) -> MemoryObject {
    MemoryObject {
        identity: Some(identity),
        extent: Some(extent),
        ..MemoryObject::new(kind)
    }
}

fn exact(object: &MemoryObject, low: i64, high: i64) -> Provenance {
    Provenance::one_with_slice(object.clone(), low, high, 1, 1, BTreeSet::new()).unwrap()
}

fn op(at: i64, operation: Operation, name: &str, defines: Vec<Value>, uses: Vec<Value>) -> Op {
    Op::new(at, OpCode::Operation(operation), name, defines, uses)
}

fn store(at: i64, uses: Vec<Value>, arg: Arg, r#ref: &MemRef) -> Op {
    Op {
        kind: Kind::Store,
        args: vec![arg],
        results: vec![cell(r#ref)],
        stores: vec![r#ref.clone()],
        ..op(at, Operation::Move, "mov", vec![], uses)
    }
}

fn load(at: i64, result: Value, uses: Vec<Value>, r#ref: &MemRef) -> Op {
    Op {
        kind: Kind::Load,
        args: vec![cell(r#ref)],
        results: vec![held(result, r#ref.width)],
        loads: vec![r#ref.clone()],
        ..op(at, Operation::Move, "mov", vec![result], uses)
    }
}

fn one_block(ops: Vec<Op>) -> MirBody {
    MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])])
}

fn at(body: &MirBody, at: i64) -> Op {
    body.blocks[0]
        .ops
        .iter()
        .find(|op| op.at == at)
        .unwrap()
        .clone()
}

fn plain(body: &MirBody) -> MirBody {
    promoted(body, &BTreeSet::new(), None, false, true, false).unwrap()
}

fn aggregate(body: &MirBody) -> MirBody {
    promoted(body, &BTreeSet::new(), None, false, true, true).unwrap()
}

fn sroa(body: &MirBody) -> MirBody {
    Sroa::new(Where::default()).transform(body.clone()).unwrap()
}

fn no_cells(op: &Op) -> bool {
    op.args.iter().all(|arg| !matches!(arg, Arg::Cell(_)))
}

#[test]
fn test_frame_promotion_respects_unknown_and_overlapping_writes() {
    for (clobber, reused) in [(None, false), (Some(-8), false), (Some(-6), true)] {
        let r#ref = MemRef::new(Some(Addr::new(Space::Frame, -8)), 2);
        let changed = match clobber {
            None => MemRef::new(None, 0),
            Some(disp) => MemRef::new(Some(Addr::new(Space::Frame, disp)), 2),
        };
        let loaded = value(1, 4);
        let first = Op {
            kind: Kind::Store,
            args: vec![constant(7, 2)],
            results: vec![cell(&r#ref)],
            stores: vec![r#ref.clone()],
            ..op(0, Operation::Move, "", vec![], vec![])
        };
        let write = Op {
            kind: Kind::Call,
            stores: vec![changed],
            memory_complete: clobber.is_some(),
            ..op(2, Operation::Call, "", vec![], vec![])
        };
        let read = Op {
            name: String::new(),
            ..load(4, loaded, vec![], &r#ref)
        };
        let result = plain(&one_block(vec![first, write, read]));
        assert_eq!(at(&result, 4).loads.is_empty(), reused, "{clobber:?}");
    }
}

#[test]
fn test_same_object_leaf_is_promoted_across_equivalent_pointer_values() {
    for far in [false, true] {
        let object_ = object(
            if far {
                MemoryKind::Named
            } else {
                MemoryKind::Frame
            },
            if far {
                Identity::Int(7)
            } else {
                Identity::Tuple(vec![
                    Identity::Str("aggregate".to_owned()),
                    Identity::Int(-16),
                ])
            },
            12,
        );
        let leaf = exact(&object_, 4, 8);
        let first = value(1, 0);
        let second = value(2, 2);
        let stored = value(3, 4);
        let loaded = value(4, 6);
        let first_segment = far.then(|| value(5, 0));
        let second_segment = far.then(|| value(6, 2));
        let space = if far { Space::Far } else { Space::Frame };
        let address = addr(space, if far { 4 } else { -12 }, if far { 7 } else { 0 });
        let via_first = MemRef {
            base: Some(first),
            segment: first_segment,
            space: Some(space),
            provenance: Some(leaf.clone()),
            ..MemRef::new(Some(address), 4)
        };
        let via_second = MemRef {
            base: Some(second),
            segment: second_segment,
            ..via_first.clone()
        };
        let first_store = Op {
            id: Some(100),
            ..store(
                4,
                [Some(stored), Some(first), first_segment]
                    .into_iter()
                    .flatten()
                    .collect(),
                held(stored, 4),
                &via_first,
            )
        };
        let second_load = Op {
            id: Some(101),
            ..load(
                6,
                loaded,
                [Some(second), second_segment]
                    .into_iter()
                    .flatten()
                    .collect(),
                &via_second,
            )
        };
        let body = one_block(vec![first_store.clone(), second_load.clone()]);

        let result = plain(&body);
        let after = at(&result, second_load.at);
        assert!(after.loads.is_empty());
        assert!(no_cells(&after));
        let scalarized = aggregate(&body);
        assert!(at(&scalarized, second_load.at).loads.is_empty());

        let scalar_object = object(
            MemoryKind::Frame,
            Identity::Tuple(vec![Identity::Str("scalar".to_owned()), Identity::Int(-2)]),
            2,
        );
        let scalar_ref = MemRef {
            space: Some(Space::Frame),
            provenance: Some(exact(&scalar_object, 0, 2)),
            ..MemRef::new(Some(Addr::new(Space::Frame, -2)), 2)
        };
        let flags = Value {
            flags: true,
            ..value(9, 8)
        };
        let scalar_update = Op {
            kind: Kind::Increment,
            args: vec![cell(&scalar_ref)],
            results: vec![cell(&scalar_ref)],
            loads: vec![scalar_ref.clone()],
            stores: vec![scalar_ref.clone()],
            id: Some(102),
            ..op(8, Operation::Unary, "inc", vec![flags], vec![])
        };
        let aggregate_flags = Value {
            flags: true,
            ..value(10, 5)
        };
        let aggregate_update = Op {
            kind: Kind::Add,
            args: vec![cell(&via_second), constant(1, 4)],
            results: vec![cell(&via_second)],
            loads: vec![via_second.clone()],
            stores: vec![via_second.clone()],
            id: Some(104),
            ..op(
                5,
                Operation::Binary,
                "add",
                vec![aggregate_flags],
                [Some(second), second_segment]
                    .into_iter()
                    .flatten()
                    .collect(),
            )
        };
        let mixed = one_block(vec![
            first_store.clone(),
            aggregate_update.clone(),
            second_load.clone(),
            scalar_update.clone(),
        ]);
        let scalarized = aggregate(&mixed);
        let aggregate_ops = scalarized.blocks[0]
            .ops
            .iter()
            .filter(|op| op.at == aggregate_update.at)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            aggregate_ops[..2]
                .iter()
                .map(|op| op.kind)
                .collect::<Vec<_>>(),
            vec![Kind::Add, Kind::Store]
        );
        assert!(aggregate_ops[0].loads.is_empty() && aggregate_ops[0].stores.is_empty());
        assert!(
            [Some(second), second_segment]
                .into_iter()
                .flatten()
                .all(|one| aggregate_ops[1].uses.contains(&one))
        );
        let untouched = scalarized.blocks[0]
            .ops
            .iter()
            .filter(|op| op.at == scalar_update.at)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            untouched,
            vec![scalar_update],
            "early SROA must not split an unrelated scalar memory update"
        );

        let integer = MemRef {
            typed: Some(("int4".to_owned(), true)),
            ..via_first.clone()
        };
        let pun = MemRef {
            typed: Some(("float4".to_owned(), false)),
            ..via_second.clone()
        };
        let typed_store = Op {
            stores: vec![integer.clone()],
            results: vec![cell(&integer)],
            ..first_store.clone()
        };
        let typed_load = Op {
            loads: vec![pun.clone()],
            args: vec![cell(&pun)],
            ..second_load.clone()
        };
        let result = aggregate(&one_block(vec![typed_store, typed_load.clone()]));
        let after = at(&result, typed_load.at);
        assert_eq!(
            after.loads,
            vec![pun],
            "incompatible union member types must keep the object in memory"
        );

        let upper_half = MemRef {
            space: Some(space),
            provenance: Some(exact(&object_, 6, 8)),
            ..MemRef::new(
                Some(addr(
                    space,
                    if far { 6 } else { -10 },
                    if far { 7 } else { 0 },
                )),
                2,
            )
        };
        let overwrite = Op {
            id: Some(103),
            ..store(6, vec![], constant(0, 2), &upper_half)
        };
        let early_value = value(7, 5);
        let early = Op {
            at: 5,
            defines: vec![early_value],
            results: vec![held(early_value, 4)],
            ..second_load.clone()
        };
        let late_value = value(8, 7);
        let late = Op {
            at: 7,
            defines: vec![late_value],
            results: vec![held(late_value, 4)],
            ..second_load.clone()
        };
        let result = aggregate(&one_block(vec![
            first_store.clone(),
            early,
            overwrite,
            late,
        ]));
        assert_eq!(
            at(&result, 5).loads,
            vec![via_second.clone()],
            "a partially overlapping object must not be scalarized"
        );
        assert_eq!(
            at(&result, 7).loads,
            vec![via_second],
            "an overlapping partial store must invalidate the scalar leaf"
        );
    }
}

#[test]
fn test_sroa_uses_a_singleton_index_range_as_an_exact_leaf() {
    let object_ = object(
        MemoryKind::Frame,
        Identity::Tuple(vec![Identity::Str("array".to_owned()), Identity::Int(-16)]),
        12,
    );
    let whole = Provenance::one(object_);
    let first = value(1, 0);
    let second = value(2, 1);
    let stored = value(3, 2);
    let loaded = value(4, 3);

    let copy = |at: i64, value: Value| Op {
        kind: Kind::Copy,
        args: vec![constant(4, 2)],
        results: vec![held(value, 2)],
        ..op(at, Operation::Move, "mov", vec![value], vec![])
    };

    let one = MemRef {
        base: Some(first),
        space: Some(Space::Frame),
        base_width: 2,
        provenance: Some(whole.clone()),
        ..MemRef::new(Some(Addr::new(Space::Frame, 0)), 4)
    };
    let two = MemRef {
        base: Some(second),
        ..one.clone()
    };
    let body = one_block(vec![
        copy(0, first),
        copy(1, second),
        store(2, vec![stored, first], held(stored, 4), &one),
        load(3, loaded, vec![second], &two),
    ]);

    let result = sroa(&body);
    let after = at(&result, 3);
    assert!(after.loads.is_empty());
    assert!(no_cells(&after));
    let interval = |low: i64, high: i64| Interval {
        low: BigInt::from(low),
        high: BigInt::from(high),
        width: 2,
    };
    assert_eq!(
        _bounded_ref(&one, &IndexMap::from_iter([(first, interval(4, 5))])).provenance,
        Some(whole.clone())
    );
    assert_eq!(
        _bounded_ref(&one, &IndexMap::from_iter([(first, interval(12, 12))])).provenance,
        Some(whole)
    );
}

fn frame_address(root: Value, offset: i64, extent: (i64, i64)) -> Op {
    Op {
        kind: Kind::Address,
        args: vec![Arg::FrameAddress(FrameAddress {
            offset,
            width: 2,
            extent: Some(extent),
        })],
        results: vec![held(root, 2)],
        ..op(0, Operation::Address, "lea", vec![root], vec![])
    }
}

fn offset(at: i64, source: Value, amount: i64, result: Value) -> Op {
    Op {
        kind: Kind::Add,
        args: vec![held(source, 2), constant(amount, 2)],
        results: vec![held(result, 2)],
        ..op(at, Operation::Binary, "add", vec![result], vec![source])
    }
}

#[test]
fn test_sroa_uses_exact_frame_pointer_provenance_as_a_leaf() {
    let root = value(1, 0);
    let first_base = value(2, 1);
    let first = value(3, 2);
    let second_base = value(4, 3);
    let second = value(5, 4);
    let loaded = value(6, 7);
    let object_ = object(
        MemoryKind::Frame,
        Identity::Tuple(vec![
            Identity::Int(7),
            Identity::Int(-16),
            Identity::Int(-4),
        ]),
        12,
    );
    let whole = Provenance::one(object_.clone());

    let reference = MemRef {
        base: Some(first),
        space: Some(Space::Frame),
        base_width: 2,
        provenance: Some(whole),
        ..MemRef::new(Some(Addr::new(Space::Literal, 0)), 4)
    };
    let equivalent = MemRef {
        base: Some(second),
        ..reference.clone()
    };
    let mut body = one_block(vec![
        frame_address(root, -16, (-16, -4)),
        offset(1, root, 16, first_base),
        offset(2, first_base, 65520, first),
        offset(3, root, 16, second_base),
        offset(4, second_base, 65520, second),
        store(5, vec![first], constant(37, 4), &reference),
        load(6, loaded, vec![second], &equivalent),
    ]);
    body.pointer_values = BTreeSet::from([root]);
    body.pointer_seeds = OrderedMap::from_iter([(root, exact(&object_, 0, 1))]);

    let result = sroa(&body);

    let after = at(&result, 6);
    assert!(after.loads.is_empty());
    assert!(no_cells(&after));
}

#[test]
fn test_sroa_refines_a_conservative_aggregate_range_from_the_exact_pointer() {
    let object_ = object(
        MemoryKind::Frame,
        Identity::Tuple(vec![Identity::Str("points".to_owned()), Identity::Int(-36)]),
        32,
    );
    let broad = Provenance::one_with_slice(object_.clone(), 2, 34, 1, 2, BTreeSet::new()).unwrap();
    let root = value(1, 0);
    let first = value(2, 1);
    let second = value(3, 2);
    let loaded = value(4, 4);

    let r#ref = MemRef {
        base: Some(first),
        space: Some(Space::Frame),
        base_width: 2,
        typed: Some(("int2".to_owned(), false)),
        provenance: Some(broad),
        ..MemRef::new(Some(Addr::new(Space::Literal, 0)), 2)
    };
    let equivalent = MemRef {
        base: Some(second),
        ..r#ref.clone()
    };
    let mut body = one_block(vec![
        frame_address(root, -36, (-36, -4)),
        offset(1, root, 6, first),
        offset(2, root, 6, second),
        store(3, vec![first], constant(29, 2), &r#ref),
        load(4, loaded, vec![second], &equivalent),
    ]);
    body.pointer_values = BTreeSet::from([root]);
    body.pointer_seeds = OrderedMap::from_iter([(root, exact(&object_, 0, 1))]);

    let result = sroa(&body);

    let after = at(&result, 4);
    assert!(after.loads.is_empty());
    assert!(no_cells(&after));
}

#[test]
fn test_sroa_matches_equivalent_affine_addresses_inside_one_dynamic_allocation() {
    let descriptor = Symbol::new(Space::Frame, 0, -38, 2);
    let root_cell = MemRef {
        space: Some(Space::Frame),
        ..MemRef::new(Some(Addr::new(Space::Frame, -28)), 2)
    };
    let narrow = value(1, 1);
    let displaced = value(2, 2);
    let extended = value(3, 3);
    let advanced = value(4, 4);
    let equivalent = value(5, 5);
    let loaded = value(6, 8);
    let allocated = Op {
        kind: Kind::Call,
        array: Some(ArrayRequest::new(descriptor, 4, vec![(0, 7)])),
        ..op(0, Operation::Call, "call", vec![], vec![])
    };

    let binary = |at: i64, kind: Kind, left: Arg, right: Arg, result: Value, width: u32| {
        let uses = [&left, &right]
            .into_iter()
            .filter_map(|arg| match arg {
                Arg::Held(held) => Some(held.value),
                _ => None,
            })
            .collect();
        Op {
            kind,
            args: vec![left, right],
            results: vec![held(result, width)],
            ..op(at, Operation::Binary, kind.as_str(), vec![result], uses)
        }
    };

    let first = binary(1, Kind::Add, cell(&root_cell), constant(0, 2), narrow, 2);
    let second = binary(2, Kind::Add, cell(&root_cell), constant(2, 2), displaced, 2);
    let widen = Op {
        kind: Kind::ZeroExtend,
        args: vec![held(displaced, 2)],
        results: vec![held(extended, 4)],
        ..op(
            3,
            Operation::Unary,
            "movzx",
            vec![extended],
            vec![displaced],
        )
    };
    let add = binary(
        4,
        Kind::Add,
        held(extended, 4),
        constant(32, 4),
        advanced,
        4,
    );
    let cancel = binary(
        5,
        Kind::Add,
        held(advanced, 4),
        constant(0xFFFF_FFDE_u32, 4),
        equivalent,
        4,
    );
    let stored_ref = MemRef {
        base: Some(narrow),
        space: Some(Space::Far),
        allocation: Some(descriptor),
        base_width: 2,
        ..MemRef::new(Some(Addr::new(Space::Far, 0)), 2)
    };
    let loaded_ref = MemRef {
        base: Some(equivalent),
        base_width: 4,
        ..stored_ref.clone()
    };
    let body = one_block(vec![
        allocated,
        first,
        second,
        widen,
        add,
        cancel,
        store(7, vec![narrow], constant(29, 2), &stored_ref),
        load(8, loaded, vec![equivalent], &loaded_ref),
    ]);

    let result = sroa(&body);

    let after = at(&result, 8);
    assert!(after.loads.is_empty());
    assert!(no_cells(&after));
}

#[test]
fn test_sroa_does_not_treat_sign_extension_as_address_preserving() {
    let r#ref = MemRef {
        space: Some(Space::Frame),
        ..MemRef::new(Some(Addr::new(Space::Frame, -4)), 2)
    };
    let narrow = value(1, 1);
    let wide = value(2, 2);
    let extend = Op {
        kind: Kind::SignExtend,
        args: vec![held(narrow, 2)],
        results: vec![held(wide, 4)],
        ..op(2, Operation::Unary, "movsx", vec![wide], vec![narrow])
    };
    let body = one_block(vec![load(1, narrow, vec![], &r#ref), extend]);

    assert!(!_affine_values(&body).contains_key(&wide));
}

#[test]
fn test_sroa_never_promotes_a_volatile_aggregate_leaf() {
    let object_ = object(
        MemoryKind::Frame,
        Identity::Tuple(vec![
            Identity::Str("volatile aggregate".to_owned()),
            Identity::Int(-8),
        ]),
        8,
    );
    let r#ref = MemRef {
        space: Some(Space::Frame),
        provenance: Some(exact(&object_, 0, 4)),
        volatile: true,
        ..MemRef::new(Some(Addr::new(Space::Frame, -8)), 4)
    };
    let loaded = value(1, 1);
    let body = one_block(vec![
        store(0, vec![], constant(7, 4), &r#ref),
        load(1, loaded, vec![], &r#ref),
    ]);

    let result = sroa(&body);

    assert_eq!(result, body);
}

#[test]
fn test_partial_store_does_not_restore_constants_from_before_unknown_effect() {
    for effect in ["call", "barrier"] {
        let whole = MemRef::new(Some(Addr::new(Space::Frame, -8)), 4);
        let word = MemRef {
            width: 2,
            ..whole.clone()
        };
        let loaded = value(1, 6);
        let initial = store(0, vec![], constant(0x1122_3344, 4), &whole);
        let clobber = Op {
            kind: if effect == "call" {
                Kind::Call
            } else {
                Kind::Opaque
            },
            ..op(
                2,
                if effect == "call" {
                    Operation::Call
                } else {
                    Operation::Barrier
                },
                "",
                vec![],
                vec![],
            )
        };
        let partial = store(4, vec![], constant(7, 2), &word);
        let read = load(6, loaded, vec![], &whole);
        let result = plain(&one_block(vec![initial, clobber, partial, read]));
        assert_eq!(at(&result, 6).loads, vec![whole], "{effect}");
    }
}

#[test]
fn test_packed_capture_keeps_wide_and_narrow_definitions_and_rejects_unknown_overlap() {
    let address = addr(Space::Segment, 6, 5);
    let whole = MemRef::new(Some(address), 4);
    let half = MemRef::new(Some(address.plus(2)), 2);
    let first = Op {
        kind: Kind::Store,
        args: vec![constant(0x1234_5678, 4)],
        stores: vec![whole.clone()],
        ..op(0, Operation::Move, "mov", vec![], vec![])
    };
    let read =
        |at: i64, r#ref: &MemRef| load(at, value(u32::try_from(at).unwrap(), at), vec![], r#ref);
    let incoming = value(100, 0);
    let overwrite = Op {
        kind: Kind::Store,
        args: vec![held(incoming, 4)],
        stores: vec![whole.clone()],
        ..op(14, Operation::Move, "mov", vec![], vec![incoming])
    };
    let body = one_block(vec![
        first.clone(),
        read(8, &whole),
        read(10, &half),
        overwrite.clone(),
        read(18, &half),
    ]);
    let result = promoted(&body, &BTreeSet::from([5]), None, false, true, false).unwrap();
    let ops = &result.blocks[0].ops;
    assert!(ops.contains(&first) && ops.contains(&overwrite));
    assert!(at(&result, 8).loads.is_empty());
    assert!(at(&result, 10).loads.is_empty());
    assert_eq!(at(&result, 18).loads, vec![half]);
    let captures = ops
        .iter()
        .filter(|op| op.at == 0 && op.kind == Kind::Copy)
        .map(|op| match op.results[0] {
            Arg::Held(held) => held.width,
            _ => unreachable!(),
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(captures, BTreeSet::from([2, 4]));
}

#[test]
fn test_split_initializer_requires_every_byte() {
    for complete in [false, true] {
        let address = addr(Space::Segment, 6, 5);
        let whole = MemRef::new(Some(address), 4);
        let word = |at: i64, offset: i64, number: i64| Op {
            kind: Kind::Store,
            args: vec![constant(number, 2)],
            stores: vec![MemRef::new(Some(address.plus(offset)), 2)],
            ..op(at, Operation::Move, "mov", vec![], vec![])
        };
        let loaded = value(10, 10);
        let stores = if complete {
            vec![word(0, 0, 0x5678), word(2, 2, 0x1234)]
        } else {
            vec![word(0, 0, 0x5678)]
        };
        let body = one_block(
            stores
                .iter()
                .cloned()
                .chain([load(10, loaded, vec![], &whole)])
                .collect(),
        );
        let result = promoted(&body, &BTreeSet::from([5]), None, false, true, false).unwrap();
        let ops = &result.blocks[0].ops;
        assert!(stores.iter().all(|op| ops.contains(op)));
        assert_eq!(at(&result, 10).loads.is_empty(), complete, "{complete}");
        if complete {
            assert!(
                ops.iter()
                    .any(|op| op.kind == Kind::Copy && op.args == vec![constant(0x1234_5678, 4)])
            );
        }
    }
}
