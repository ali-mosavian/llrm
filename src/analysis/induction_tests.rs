//! Ports of `tests/test_induction_identity.py` and
//! `tests/test_induction_inequality.py`.
//!
//! Skipped, needing `lower`, `strength`, `transform`, `wholeseg` or the
//! corpus: `test_nine_dimensional_loop_carries_its_pointer`,
//! `test_native_array_helper_does_not_block_frame_forwarding`,
//! `test_huge_loop_byte_offsets_are_induction_variables`,
//! `test_huge_loop_carries_whole_pointers`,
//! `test_sign_extended_recurrence_requires_no_narrow_wrap`,
//! `test_zero_extended_recurrence_cannot_cross_unsigned_wrap`,
//! `test_zero_extended_counter_product_is_carried_as_a_wide_recurrence`,
//! `test_matrix_reduced_stride_keeps_its_multiplier_address`,
//! `test_harr_hoisted_descriptor_read_keeps_its_address`,
//! `test_nbody_inner_counter_has_a_proven_upper_bound`,
//! `test_harr_stored_row_plus_column_is_loop_carried`,
//! `test_harr_descriptor_offset_is_read_before_inner_loop_unless_written`,
//! `test_nested_address_advances_instead_of_recomputing_row_plus_column`,
//! `test_strength_does_not_spill_cheap_loop_work`,
//! `test_lngmxx_accumulator_has_a_whole_long_start`,
//! `test_a_reduced_counter_has_its_own_loop_phi_and_fresh_variable`,
//! `test_reduction_preserves_every_live_product_result`,
//! `test_reduction_does_not_speculate_on_a_loop_bypass`,
//! `test_existing_phi_inputs_follow_their_predecessor_versions`,
//! `test_reduced_product_keeps_the_current_iteration_on_exit`,
//! `test_inserted_counter_operations_own_their_insertion_location`,
//! `test_cse_replaces_phi_uses_of_a_deleted_initializer`,
//! `test_cse_keeps_distinct_linker_addresses`,
//! `test_dead_byte_transfer_cannot_span_a_surviving_jump`,
//! `test_nested_row_recurrences_remove_repeated_multiplication`,
//! `test_not_equal_loop_reaches_bound_without_wrapping`.
//! The two posttested trip-count tests keep their `trip_count` half; the
//! `lower.lowered` half is skipped, as is the `strength.reduced` half of
//! `test_composed_offset_can_carry_an_invariant_pointer`.

use std::rc::Rc;
use std::collections::BTreeSet;

use indexmap::IndexMap;
use num_bigint::BigInt;

use super::{_counter_bound, Affine, AffineMap, AffineOperand, Derived, basics, derived, relation, trip_count};
use crate::analysis::consts;
use crate::analysis::loops::Loop;
use crate::analysis::occurrence::operations;
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Const, Held, Kind, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value};

fn value(id: u32, at: i64, variable: u32) -> Value {
    Value {
        variable,
        ..Value::new(id, at)
    }
}

fn flags(id: u32, at: i64) -> Value {
    Value {
        flags: true,
        ..Value::new(id, at)
    }
}

fn op(at: i64, operation: Operation, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
    let mut made = Op::new(at, OpCode::Operation(operation), "", defines, uses);
    made.kind = kind;
    made
}

fn held(value: Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

fn constant(n: i64, width: u32) -> AffineOperand {
    AffineOperand::Const(Const::new(n, width))
}

fn looped(header: i64, latches: &[i64], body: &[i64]) -> Loop {
    Loop {
        header,
        latches: latches.iter().copied().collect(),
        body: body.iter().copied().collect(),
    }
}

fn derive(body: &MirBody, loop_: &Loop) -> Vec<Derived> {
    derived(&Rc::new(body.clone()), loop_, None, &BTreeSet::new(), None).unwrap()
}

#[test]
fn test_counter_zero_test_requires_an_unchanged_counter() {
    for (kind, same, accepted) in [
        (Kind::Or, true, true),
        (Kind::And, true, true),
        (Kind::Xor, true, false),
        (Kind::Or, false, false),
        (Kind::And, false, false),
    ] {
        let (value, result, flag) = (Value::new(900, 0), Value::new(901, 0), flags(902, 0));
        let source = held(value, 2);
        let mut test = op(0, Operation::Binary, vec![result, flag], vec![value], kind);
        test.args = vec![source.clone(), if same { source } else { Arg::Const(Const::new(1, 2)) }];
        test.results = vec![held(result, 2)];
        let branch = op(1, Operation::Branch, vec![], vec![flag], Kind::Branch);
        let counter = Affine {
            value: value.id,
            start: constant(-1, 2),
            step: constant(1, 2),
            header: 0,
        };
        assert_eq!(
            _counter_bound(&test, &branch, &counter, 2, None),
            accepted.then(|| Arg::Const(Const::new(0, 2)))
        );
    }
}

#[test]
fn test_counter_zero_test_keeps_its_flags_across_a_partial_result() {
    let (value, result, flag) = (Value::new(910, 0), Value::new(911, 0), flags(912, 0));
    let source = held(value, 2);
    let mut test = op(0, Operation::Binary, vec![result, flag], vec![value], Kind::Or);
    test.merges = OrderedMap::from_iter([(value, result)]);
    test.args = vec![source.clone(), source];
    test.results = vec![held(result, 2)];
    let branch = op(1, Operation::Branch, vec![], vec![flag], Kind::Branch);
    let counter = Affine {
        value: value.id,
        start: constant(-1, 2),
        step: constant(1, 2),
        header: 0,
    };
    assert_eq!(_counter_bound(&test, &branch, &counter, 2, None), Some(Arg::Const(Const::new(0, 2))));
}

#[test]
fn test_affine_map_carries_the_modular_injectivity_proof() {
    let source = Affine {
        value: 1,
        start: constant(0, 2),
        step: constant(1, 2),
        header: 1,
    };
    let byte_offset = Affine {
        value: 2,
        start: constant(0, 2),
        step: constant(16, 2),
        header: 1,
    };

    let mapping = relation(&source, &byte_offset, &IndexMap::new()).unwrap();

    assert_eq!(
        mapping,
        AffineMap {
            scale: BigInt::from(16),
            offset: BigInt::from(0),
            width: 2
        }
    );
    assert!(mapping.injective(&BigInt::from(0), &BigInt::from(5)));
    assert!(!mapping.injective(&BigInt::from(0), &BigInt::from(4096)));
}

#[test]
fn test_posttested_counter_has_an_exact_fixed_trip_count() {
    let start = value(920, 0, 1);
    let counter = value(921, 1, 1);
    let following = value(922, 1, 1);
    let flag = flags(923, 2);
    let mut initial = op(0, Operation::Move, vec![start], vec![], Kind::Copy);
    initial.args = vec![Arg::Const(Const::new(0, 2))];
    initial.results = vec![held(start, 2)];
    let mut increment = op(1, Operation::Unary, vec![following], vec![counter], Kind::Increment);
    increment.args = vec![held(counter, 2)];
    increment.results = vec![held(following, 2)];
    let mut compare = op(2, Operation::Binary, vec![flag], vec![following], Kind::Sub);
    compare.args = vec![held(following, 2), Arg::Const(Const::new(4, 2))];
    let mut branch = op(3, Operation::Branch, vec![], vec![flag], Kind::Branch);
    branch.test = Some(Kind::AboveEq);
    branch.target = Some(3);
    let body = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![initial], vec![1]),
            MirBlock::new(
                1,
                vec![Phi {
                    result: counter,
                    incoming: OrderedMap::from_iter([(0, start), (2, following)]),
                }],
                vec![increment],
                vec![2],
            ),
            MirBlock::new(2, vec![], vec![compare, branch], vec![1, 3]),
            MirBlock::new(3, vec![], vec![], vec![]),
        ],
    );
    let loop_ = looped(1, &[2], &[1, 2]);
    let facts = consts::known(&Rc::new(MirBody::clone(&body)), None, None, None, None);
    assert_eq!(trip_count(&body, &loop_, &facts), Some(BigInt::from(4)));
}

#[test]
fn test_posttested_symbolic_sentinel_keeps_its_exact_trip_count() {
    let start = value(930, 0, 1);
    let end = value(931, 0, 2);
    let counter = value(932, 1, 3);
    let following = value(933, 2, 3);
    let flag = flags(934, 2);
    let mut endpoint = op(0, Operation::Binary, vec![end], vec![start], Kind::Add);
    endpoint.args = vec![held(start, 4), Arg::Const(Const::new(768, 4))];
    endpoint.results = vec![held(end, 4)];
    let mut increment = op(2, Operation::Binary, vec![following], vec![counter], Kind::Add);
    increment.args = vec![held(counter, 4), Arg::Const(Const::new(24, 4))];
    increment.results = vec![held(following, 4)];
    let mut compare = op(2, Operation::Binary, vec![flag], vec![following, end], Kind::Sub);
    compare.args = vec![held(following, 4), held(end, 4)];
    let mut branch = op(2, Operation::Branch, vec![], vec![flag], Kind::Branch);
    branch.test = Some(Kind::Ne);
    branch.target = Some(1);
    let body = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![endpoint], vec![1]),
            MirBlock::new(
                1,
                vec![Phi {
                    result: counter,
                    incoming: OrderedMap::from_iter([(0, start), (2, following)]),
                }],
                vec![],
                vec![2],
            ),
            MirBlock::new(2, vec![], vec![increment, compare, branch], vec![1, 3]),
            MirBlock::new(3, vec![], vec![], vec![]),
        ],
    );
    let loop_ = looped(1, &[2], &[1, 2]);
    let facts = consts::known(&Rc::new(MirBody::clone(&body)), None, None, None, None);
    assert_eq!(trip_count(&body, &loop_, &facts), Some(BigInt::from(32)));
}

fn body() -> (MirBody, Loop) {
    let start = value(10, 0, 7);
    let counter = value(11, 1, 7);
    let following = value(12, 1, 7);
    let unrelated = value(13, 1, 7);
    let answer = value(14, 1, 8);
    let mut step = op(1, Operation::Unary, vec![following], vec![counter], Kind::Increment);
    step.args = vec![held(counter, 2)];
    step.results = vec![held(following, 2)];
    let mut multiply = op(2, Operation::Binary, vec![answer], vec![unrelated], Kind::Mul);
    multiply.args = vec![held(unrelated, 2), Arg::Const(Const::new(2, 2))];
    multiply.results = vec![held(answer, 2)];
    let blocks = vec![
        MirBlock::new(0, vec![], vec![], vec![1]),
        MirBlock::new(
            1,
            vec![Phi {
                result: counter,
                incoming: OrderedMap::from_iter([(0, start), (1, following)]),
            }],
            vec![step, multiply],
            vec![1, 2],
        ),
        MirBlock::new(2, vec![], vec![], vec![]),
    ];
    (MirBody::new(0, blocks), looped(1, &[1], &[1]))
}

#[test]
fn test_a_shared_variable_name_is_not_a_shared_recurrence() {
    let (built, loop_) = body();
    assert!(!basics(&built, &loop_).is_empty());
    assert!(derive(&built, &loop_).is_empty());
    let counter = built.blocks[1].phis[0].result;
    let mut positive = built.clone();
    let multiply = &mut positive.blocks[1].ops[1];
    multiply.uses = vec![counter];
    multiply.args = vec![held(counter, 2), Arg::Const(Const::new(2, 2))];
    assert_eq!(derive(&positive, &loop_).len(), 1);
}

#[test]
fn test_the_backedge_must_step_the_exact_phi_value() {
    let (mut built, loop_) = body();
    let unrelated = built.blocks[1].ops[1].args[0].clone();
    let Arg::Held(Held { value: unrelated_value, .. }) = unrelated else {
        unreachable!("the multiply reads a value");
    };
    let step = &mut built.blocks[1].ops[0];
    step.args = vec![unrelated];
    step.uses = vec![unrelated_value];
    assert!(basics(&built, &loop_).is_empty());
}

#[test]
fn test_long_recurrence_keeps_its_width() {
    for copied in [false, true] {
        let (mut built, loop_) = body();
        let mut update = built.blocks[1].ops[0].clone();
        update.args = vec![held(update.uses[0], 4)];
        update.results = vec![held(update.defines[0], 4)];
        let mut ops = vec![update.clone()];
        if copied {
            let temporary = Value::new(100, 1);
            let original = update.defines[0];
            update.defines = vec![temporary];
            update.results = vec![held(temporary, 4)];
            let mut copy = op(3, Operation::Move, vec![original], vec![temporary], Kind::Copy);
            copy.args = vec![held(temporary, 4)];
            copy.results = vec![held(original, 4)];
            ops = vec![update, copy];
        }
        built.blocks[1].ops = ops;
        let recurrences = basics(&built, &loop_);
        let recurrence = recurrences.get(&built.blocks[1].phis[0].result.id).unwrap();
        assert_eq!(recurrence.start.width(), 4);
        assert_eq!(recurrence.step, constant(1, 4));
    }
}

#[test]
fn test_composed_word_address_has_one_recurrence() {
    for (factor, expected) in [(20, 42), (32767, 0)] {
        let (mut built, loop_) = body();
        let header = built.blocks[1].clone();
        let counter = header.phis[0].result;
        let Arg::Held(Held { value: product, .. }) = header.ops[1].results[0] else {
            unreachable!("the multiply has a value");
        };
        let summed = value(30, 1, 30);
        let address = value(31, 1, 31);
        let factor_value = value(32, 0, 32);
        let mut constant_op = header.ops[1].clone();
        constant_op.kind = Kind::Copy;
        constant_op.defines = vec![factor_value];
        constant_op.uses = vec![];
        constant_op.args = vec![Arg::Const(Const::new(factor, 2))];
        constant_op.results = vec![held(factor_value, 2)];
        let mut multiply = header.ops[1].clone();
        multiply.uses = vec![counter, factor_value];
        multiply.args = vec![held(counter, 2), held(factor_value, 2)];
        let mut add = multiply.clone();
        add.kind = Kind::Add;
        add.defines = vec![summed];
        add.uses = vec![product, counter];
        add.args = vec![held(product, 2), held(counter, 2)];
        add.results = vec![held(summed, 2)];
        let mut shift = multiply.clone();
        shift.kind = Kind::Shl;
        shift.defines = vec![address];
        shift.uses = vec![summed];
        shift.args = vec![held(summed, 2), Arg::Const(Const::new(1, 2))];
        shift.results = vec![held(address, 2)];
        built.blocks[0].ops = vec![constant_op];
        built.blocks[1].ops = vec![header.ops[0].clone(), multiply, add, shift];
        let (occurrence, _, _) = operations(&built)
            .find(|(_, block, one)| block.at == 1 && one.kind == Kind::Shl)
            .unwrap();
        let found = derive(&built, &loop_);
        let formula = found.iter().find(|one| one.op == occurrence).unwrap();
        assert_eq!(formula.by, Arg::Const(Const::new(expected, 2)));
    }
}

#[test]
fn test_composed_offset_can_carry_an_invariant_pointer() {
    let (mut built, loop_) = body();
    let header = built.blocks[1].clone();
    let counter = header.phis[0].result;
    let following = header.ops[0].defines[0];
    let mut step = header.ops[0].clone();
    step.args = vec![held(counter, 4)];
    step.results = vec![held(following, 4)];
    let offset = header.ops[1].defines[0];
    let mut multiply = header.ops[1].clone();
    multiply.uses = vec![counter];
    multiply.args = vec![held(counter, 4), Arg::Const(Const::new(2, 4))];
    multiply.results = vec![held(offset, 4)];
    let base = value(40, 0, 40);
    let pointer = value(41, 1, 41);
    let displaced = value(42, 1, 42);
    let mut add = op(3, Operation::Binary, vec![displaced], vec![offset], Kind::Add);
    add.args = vec![held(offset, 4), Arg::Const(Const::new(6, 4))];
    add.results = vec![held(displaced, 4)];
    let mut address = op(4, Operation::Binary, vec![pointer], vec![base, displaced], Kind::PtrOffset);
    address.args = vec![held(base, 4), held(displaced, 4)];
    address.results = vec![held(pointer, 4)];
    let phi = Phi {
        result: header.phis[0].result,
        incoming: OrderedMap::from_iter([(0, *header.phis[0].incoming.get(&0).unwrap()), (1, following)]),
    };
    built.blocks[1].phis = vec![phi];
    built.blocks[1].ops = vec![step, multiply, add, address];
    let (occurrence, _, _) = operations(&built)
        .find(|(_, _, one)| one.kind == Kind::PtrOffset)
        .unwrap();
    let found = derive(&built, &loop_);
    let carried = found.iter().find(|one| one.op == occurrence).unwrap();
    assert_eq!(carried.pointer, Some(held(base, 4)));
    assert_eq!(carried.of.value, counter.id);
    assert_eq!(carried.by, Arg::Const(Const::new(2, 4)));
    assert_eq!(carried.offsets, vec![(Arg::Const(Const::new(6, 4)), BigInt::from(1))]);
}

#[test]
fn test_every_incoming_path_agrees_on_the_recurrence() {
    for mismatch in ["start", "step", "unchanged", "none"] {
        let (mut built, loop_) = body();
        let header = built.blocks[1].clone();
        let phi = &header.phis[0];
        let start = *phi.incoming.get(&0).unwrap();
        let following = value(20, 3, 7);
        let mut step = header.ops[0].clone();
        step.at = 3;
        step.defines = vec![following];
        step.results = vec![held(following, 2)];
        if mismatch == "step" {
            step.kind = Kind::Decrement;
        }
        let mut incoming = phi.incoming.clone();
        incoming.insert(4, if mismatch == "start" { value(21, 4, 7) } else { start });
        incoming.insert(3, if mismatch == "unchanged" { phi.result } else { following });
        built.blocks[1].phis = vec![Phi {
            result: phi.result,
            incoming,
        }];
        built.blocks[1].succ = vec![1, 2, 3];
        built.blocks.push(MirBlock::new(3, vec![], vec![step], vec![1]));
        built.blocks.push(MirBlock::new(4, vec![], vec![], vec![1]));
        let loop_ = Loop {
            body: [1, 3].into_iter().collect(),
            latches: [1, 3].into_iter().collect(),
            ..loop_
        };
        assert_eq!(!basics(&built, &loop_).is_empty(), mismatch == "none", "{mismatch}");
    }
}

#[test]
fn test_only_width_preserving_copies_carry_the_recurrence() {
    for (width, count) in [(2, 1), (4, 0)] {
        let (mut built, loop_) = body();
        let counter = built.blocks[1].phis[0].result;
        let Arg::Held(Held { value: copied, .. }) = built.blocks[1].ops[1].args[0] else {
            unreachable!("the multiply reads a value");
        };
        let mut copy = op(1, Operation::Move, vec![copied], vec![counter], Kind::Copy);
        copy.args = vec![held(counter, width)];
        copy.results = vec![held(copied, 2)];
        built.blocks[1].ops.insert(0, copy);
        assert_eq!(derive(&built, &loop_).len(), count);
    }
}

#[test]
fn test_a_shift_recurrence_requires_a_constant_count() {
    for shape in ["variable", "counter_count", "constant", "oversized"] {
        let (mut built, loop_) = body();
        let counter = held(built.blocks[1].phis[0].result, 2);
        let mut amount = if shape == "constant" {
            Arg::Const(Const::new(3, 2))
        } else {
            held(*built.blocks[1].phis[0].incoming.get(&0).unwrap(), 2)
        };
        if shape == "oversized" {
            amount = Arg::Const(Const::new(32, 2));
        }
        let args = if shape == "counter_count" {
            vec![Arg::Const(Const::new(3, 2)), counter]
        } else {
            vec![counter, amount]
        };
        let shift = &mut built.blocks[1].ops[1];
        shift.kind = Kind::Shl;
        shift.uses = args
            .iter()
            .filter_map(|one| match one {
                Arg::Held(held) => Some(held.value),
                _ => None,
            })
            .collect();
        shift.args = args;
        let found = derive(&built, &loop_);
        assert_eq!(found.len(), usize::from(shape == "constant"), "{shape}");
        if let Some(formula) = found.first() {
            assert_eq!(formula.by, Arg::Const(Const::new(8, 2)));
        }
    }
}
