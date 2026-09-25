//! Port of `tests/test_raising_arrays.py`.
//!
//! Also ports `tests/test_array_bounds.py`'s
//! `test_nine_dimensional_loop_proves_its_element_store_extent`. The rest of
//! that file is skipped, monkeypatching `raising_array_bounds.proven` out of
//! `mir.bodies`:
//! `test_unknown_logical_loop_condition_proves_no_array_extent`,
//! `test_huge_loop_descriptor_loads_leave_the_loop`,
//! `test_huge_proof_does_not_assume_its_own_disjointness`,
//! `test_harr_dimension_survives_element_stores`,
//! `test_failed_path_proof_discards_all_disjointness`.

use std::collections::BTreeSet;

use super::*;
use crate::model::ir::Operation;
use crate::model::mir::{self, MirBlock, MirBody, Op, OpCode};
use crate::testing::{self, nth, ops, overlapping};

const TAGS: [&str; 3] = ["p-g2", "q-O", "v-g3"];

/// HARR's 21-element dimensions were unknown immediately after DDIM returned.
#[test]
fn test_dim_normal_return_supplies_descriptor_constants() {
    for tag in TAGS {
        let path = format!("tests/fixtures/omf/harr-{tag}.obj").to_lowercase();
        let found = testing::loaded(&path).unwrap();
        let body = nth(&testing::raised(&path), 0);
        let call = ops(&body).into_iter().find(|op| op.array.is_some()).unwrap();
        let (dgroup, none) = (&found.dgroup.members, IndexMap::default());
        let facts = consts::_kills(Default::default(), &call, &none, dgroup, &found.calls, None, None, false, None);
        let field = |disp: i64| MemRef::new(Some(Addr { index: found.program_data.unwrap(), ..Addr::new(Space::Segment, disp) }), 2);
        assert_eq!(consts::_cell(&facts, &field(20)), Some(consts::Known::new(21, 2)), "{tag}");
        assert_eq!(consts::_cell(&facts, &field(24)), Some(consts::Known::new(21, 2)), "{tag}");
        let mut unknown = call.clone();
        unknown.kind = Kind::Store;
        unknown.memory_values = vec![];
        assert!(consts::_kills(facts, &unknown, &none, dgroup, &IndexMap::default(), None, None, false, None).is_empty(), "{tag}");
    }
}

/// HARR's descriptor fields looked like arbitrary pointer accesses to alias analysis.
#[test]
fn test_descriptor_fields_have_proven_addresses_without_new_relocations() {
    for tag in TAGS {
        let path = format!("tests/fixtures/omf/harr-{tag}.obj").to_lowercase();
        let found = testing::loaded(&path).unwrap();
        let body = nth(&testing::raised(&path), 0);
        let fields: Vec<MemRef> =
            ops(&body).into_iter().flat_map(|op| op.loads).filter(|one| one.symbolic.is_some()).collect();
        let offsets: BTreeSet<i64> = fields.iter().map(|one| one.symbolic.as_ref().unwrap().offset).collect();
        assert!(offsets.is_superset(&BTreeSet::from([8, 16])), "{tag}");
        for reference in &fields {
            let addr = reference.addr.unwrap();
            assert!(addr.space == Space::Literal && reference.base.is_some(), "{tag}");
            let symbol = reference.symbolic.as_ref().unwrap();
            let direct = MemRef::new(Some(Addr { index: symbol.index, ..Addr::new(Space::Segment, symbol.offset) }), reference.width);
            assert!(mir::same_bytes(reference, &direct), "{tag}");
            assert!(overlapping(reference, &direct, Some(&found.dgroup)), "{tag}");
            let unrelated = MemRef {
                addr: Some(Addr { index: symbol.index, ..Addr::new(Space::Segment, symbol.offset + i64::from(reference.width)) }),
                ..direct.clone()
            };
            assert!(!overlapping(reference, &unrelated, Some(&found.dgroup)), "{tag}");
        }
    }
}

#[test]
fn test_real_array_requests() {
    for tag in TAGS {
        for (name, bounds) in [("harr", vec![(0, 20), (0, 20)]), ("segld", vec![(0, 100)])] {
            let path = format!("tests/fixtures/omf/{name}-{tag}.obj").to_lowercase();
            let found = testing::loaded(&path).unwrap();
            let requests: Vec<ArrayRequest> =
                testing::all_ops(&testing::raised(&path)).into_iter().filter_map(|op| op.array).collect();
            assert_eq!(requests.len(), 1, "{name} {tag}");
            let request = &requests[0];
            assert_eq!(request.bounds, bounds, "{name} {tag}");
            assert_eq!(request.element_width, 2, "{name} {tag}");
            assert_eq!(Some(request.descriptor.index), found.program_data, "{name} {tag}");
            assert!(!request.replaces, "{name} {tag}");
        }
    }
}

/// NDARR's OR-based zero test defeated the extent proof despite valid 1,12,2 output.
#[test]
fn test_nine_dimensional_loop_proves_its_element_store_extent() {
    for tag in TAGS {
        let body = nth(&testing::raised(format!("tests/fixtures/regressions/ndarr-{tag}.obj").to_lowercase()), 0);
        let stores: Vec<MemRef> = ops(&body).into_iter().flat_map(|op| op.stores).filter(|one| one.pointer).collect();
        assert!(!stores.is_empty(), "{tag}");
        assert!(stores.iter().all(|one| one.allocation.is_some()), "{tag}");
    }
}

/// Unequal dimensions must not be swapped: the last pushed bound lives at descriptor +14.
#[test]
fn test_descriptor_dimensions_follow_stack_order() {
    let request = ArrayRequest::new(Symbol::new(Space::Segment, 5, 6, 2), 2, vec![(-3, 2), (4, 14)]);
    let mut arguments: Vec<Option<Arg>> =
        [-3, 2, 4, 14, 2, 2].into_iter().map(|number| Some(Arg::Const(Const::new(number, 2)))).collect();
    arguments.push(Some(Arg::Symbol(request.descriptor)));
    let fields = _descriptor_values(Some(&request), &arguments, "qb45");
    let got: Vec<(i64, BigInt)> = fields.iter().map(|(reference, value)| (reference.addr.unwrap().disp, value.n.clone())).collect();
    let expected: Vec<(i64, BigInt)> =
        [(14, 2), (15, 0), (18, 2), (20, 11), (22, 4), (24, 6), (26, -3)].into_iter().map(|(at, n)| (at, n.into())).collect();
    assert_eq!(got, expected);
    assert!(_descriptor_values(Some(&request), &arguments, "unknown").is_empty());
    let at = arguments.len() - 2;
    arguments[at] = Some(Arg::Const(Const::new(0x8002, 2)));
    assert!(_descriptor_values(Some(&request), &arguments, "vbdos").is_empty());
}

/// Runtime-sized DIM still establishes rank, numeric allocation kind and element width.
#[test]
fn test_unknown_bounds_still_establish_descriptor_shape() {
    let descriptor = Symbol::new(Space::Segment, 5, 6, 2);
    let arguments = vec![
        None,
        None,
        None,
        None,
        Some(Arg::Const(Const::new(2, 2))),
        Some(Arg::Const(Const::new(0x102, 2))),
        Some(Arg::Symbol(descriptor)),
    ];
    assert!(_request(&arguments, false).is_none());
    let fields = _descriptor_values(None, &arguments, "pds71");
    let got: Vec<(i64, BigInt)> = fields.iter().map(|(reference, value)| (reference.addr.unwrap().disp, value.n.clone())).collect();
    assert_eq!(got, vec![(14, 2.into()), (15, 1.into()), (18, 2.into())]);
}

#[test]
fn test_unknown_segment_wrapping_or_wide_pointer_is_not_resolved() {
    for (space, offset, width) in [(Space::Far, 6, 2), (Space::Literal, 65535, 2), (Space::Literal, 6, 4)] {
        let pointer = Value::new(1, 0);
        let mut reference = MemRef::new(Some(Addr::new(space, 2)), 2);
        reference.base = Some(pointer);
        let mut op = Op::new(0, OpCode::Operation(Operation::Move), "mov", vec![], vec![pointer]);
        op.loads = vec![reference.clone()];
        op.args = vec![Arg::Cell(Cell { r#ref: reference })];
        let body = RaisedBody::new(MirBody::new(0, vec![MirBlock::new(0, vec![], vec![op], vec![])]));
        let symbols = IndexMap::from_iter([(pointer, Symbol::new(Space::Segment, 5, offset, width))]);
        let result = _addresses(body, &symbols);
        assert!(result.blocks[0].ops[0].loads[0].symbolic.is_none(), "{space:?} {offset} {width}");
    }
}

#[test]
fn test_only_allocating_calls_carry_requests() {
    for name in ["B$DDIM", "B$RDIM", "B$ADIM", "unknown"] {
        let descriptor = Symbol::new(Space::Segment, 5, 6, 2);
        let args = [
            Arg::Const(Const::new(-2, 2)),
            Arg::Const(Const::new(3, 2)),
            Arg::Const(Const::new(4, 2)),
            Arg::Const(Const::new(257, 2)),
            Arg::Symbol(descriptor),
        ];
        let pushes: Vec<Op> = args
            .iter()
            .enumerate()
            .map(|(at, arg)| {
                let mut push = Op::new(at as i64, OpCode::Operation(Operation::Push), "push", vec![], vec![]);
                push.kind = Kind::Arg;
                push.args = vec![arg.clone()];
                push
            })
            .collect();
        let mut call = Op::new(5, OpCode::Operation(Operation::Call), "call", vec![], vec![]);
        call.kind = Kind::Call;
        let ops: Vec<Op> = pushes.iter().cloned().chain([call.clone()]).collect();
        let body = RaisedBody::new(MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])]));
        let calls = IndexMap::from_iter([(5, name.to_owned())]);
        let result = annotated(body.clone(), &calls, "").blocks[0].ops.last().unwrap().array.clone();
        if matches!(name, "B$DDIM" | "B$RDIM") {
            let expected = ArrayRequest { descriptor, element_width: 4, bounds: vec![(-2, 3)], replaces: name == "B$RDIM" };
            assert_eq!(result, Some(expected), "{name}");
        } else {
            assert!(result.is_none(), "{name}");
        }
        let interrupted: Vec<Op> = pushes.iter().cloned().chain([Op { at: 4, ..call.clone() }, call]).collect();
        let interrupted = body.with_blocks(vec![body.blocks[0].with_ops(interrupted)]);
        let calls = IndexMap::from_iter([(4, "unknown".to_owned()), (5, name.to_owned())]);
        assert!(annotated(interrupted, &calls, "").blocks[0].ops.last().unwrap().array.is_none(), "{name}");
    }
}
