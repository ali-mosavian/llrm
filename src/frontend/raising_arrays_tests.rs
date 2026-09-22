//! Port of `tests/test_raising_arrays.py`.
//!
//! Skipped, needing `mir.bodies` and corpus loaders:
//! `test_dim_normal_return_supplies_descriptor_constants`,
//! `test_descriptor_fields_have_proven_addresses_without_new_relocations`,
//! `test_real_array_requests`.
//!
//! `tests/test_array_bounds.py` is skipped whole, needing `mir.bodies`,
//! corpus loaders and `wholeseg`:
//! `test_nine_dimensional_loop_proves_its_element_store_extent`,
//! `test_unknown_logical_loop_condition_proves_no_array_extent`,
//! `test_huge_loop_descriptor_loads_leave_the_loop`,
//! `test_huge_proof_does_not_assume_its_own_disjointness`,
//! `test_harr_dimension_survives_element_stores`,
//! `test_failed_path_proof_discards_all_disjointness`.

use super::*;
use crate::model::ir::Operation;
use crate::model::mir::{MirBlock, MirBody, Op, OpCode};

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
