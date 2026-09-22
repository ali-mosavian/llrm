//! Ports of `tests/test_array_access.py` and `tests/test_huge_array_access.py`.
//!
//! Skipped, needing `mir.bodies`, corpus loaders, `wholeseg` or `rewrite`:
//! `test_checked_constant_indices_can_use_native_addressing`,
//! `test_checked_access_proofs_reach_fixed_point`,
//! `test_hary_supports_nine_and_sixty_dimensions`,
//! `test_native_array_arithmetic_keeps_allocation_dimension_constants`,
//! `test_overflow_observation_has_no_normal_path_register_results`,
//! `test_array_checks_are_independent_of_numeric_semantics`,
//! `test_bounds_policy_is_recorded_separately`,
//! `test_unsupported_checked_helper_is_not_unchecked_success`,
//! `test_dynamic_far_address_arithmetic_is_native`,
//! `test_dynamic_shape_does_not_freeze_descriptor_fields`,
//! `test_dynamic_unestablished_layout_is_not_assumed_far`,
//! `test_dynamic_address_uses_runtime_lower_bounds_and_correct_stride`,
//! `test_huge_helper_becomes_whole_pointer_mir_and_emitted_accesses`,
//! `test_selector_proof_checks_the_loop_exit_path`.

use super::*;

/// An out-of-range dimension can flatten into a valid allocation offset; that is still a bounds error.
#[test]
fn test_checked_proof_requires_each_live_dimension() {
    for hazard in ["none", "below", "above", "unknown-index", "rank", "features", "width", "unknown-bound"] {
        let symbol = Symbol::new(Space::Segment, 5, 0, 2);
        let field = |offset: i64, width: u32| MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, offset) }), width);
        let cell = |offset: i64| Arg::Cell(Cell { r#ref: field(offset, 2) });
        let shape = Descriptor {
            data: cell(0),
            selector: field(2, 2),
            width: 2,
            dimensions: vec![(cell(14), cell(16)), (cell(18), cell(20))],
            huge: false,
        };
        let mut memory = consts::Cells::default();
        for (offset, width, number) in [(8, 1, 2), (9, 1, 1), (12, 2, 2), (14, 2, 2), (16, 2, 0xffff), (18, 2, 3), (20, 2, 2)] {
            if hazard == "unknown-bound" && offset == 18 {
                continue;
            }
            let zeroed = match hazard {
                "rank" => Some(8),
                "features" => Some(9),
                "width" => Some(12),
                _ => None,
            };
            let number = if zeroed == Some(offset) { 0 } else { number };
            memory.extend(consts::_fragments(&field(offset, width), &consts::Known::new(number, width)));
        }
        let mut indices = vec![Some(consts::Known::new(3, 2)), Some(consts::Known::new(0xffff, 2))];
        match hazard {
            "below" => indices[1] = Some(consts::Known::new(0xfffe, 2)),
            "above" => indices[1] = Some(consts::Known::new(1, 2)),
            "unknown-index" => indices[1] = None,
            _ => {}
        }
        assert_eq!(_checked(Some(&shape), Some(&symbol), &indices, &memory), hazard == "none", "{hazard}");
    }
}

/// Removing HUGELP's dead offset merges must not erase high bits a later whole read uses.
#[test]
fn test_offset_overwrite_keeps_a_transitively_observed_high_half() {
    let value = |index: u32| Value { variable: index, ..Value::new(index, i64::from(index)) };
    let (old, first, second) = (value(1), value(2), value(3));
    let copies: Vec<Op> = [(0, old, first), (1, first, second)]
        .into_iter()
        .map(|(index, source, result)| {
            let mut copy = Op::new(index, OpCode::Operation(Operation::Move), "mov", vec![result], vec![source]);
            copy.kind = Kind::Copy;
            copy.args = vec![Arg::Const(Const::new(index, 2))];
            copy.results = vec![Arg::Held(Held { value: result, width: 2 })];
            copy.merges = OrderedMap::from_iter([(source, result)]);
            copy
        })
        .collect();
    let mut read = Op::new(2, OpCode::Operation(Operation::Push), "push", vec![], vec![second]);
    read.kind = Kind::Arg;
    read.args = vec![Arg::Held(Held { value: second, width: 4 })];
    let ops: Vec<Op> = copies.iter().cloned().chain([read]).collect();
    let body = MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])]);
    assert!(!_overwrites_offset(&body, &copies[0], old));
}
