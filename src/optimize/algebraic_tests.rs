//! Port of `tests/test_algebraic.py`.
//!
//! Skipped, needing modules not yet ported:
//! - corpus fixtures, `mir.bodies`, `wholeseg`, `transform`:
//!   test_nbody_reuses_the_whole_signed_initialization_value,
//!   test_signed_recombination_requires_the_exact_extension,
//!   test_nbody_counter_comparison_joins_whole_values_before_the_loop,
//!   test_nbody_counter_is_stored_as_one_whole_value,
//!   test_whole_store_requires_adjacent_matching_word_writes,
//!   test_whole_counter_phi_requires_every_matching_edge,
//!   test_sixty_dimensional_zero_offset_needs_no_pointer_arithmetic,
//!   test_hotlpx_scales_by_twenty_without_a_second_multiply,
//!   test_spill_folds_closed_loops_to_their_exact_final_constants,
//!   test_addrm_reuses_word_scale_for_long_address,
//!   test_nested_combines_row_scale_in_emitted_code,
//!   test_nbody_multiply_value_survives_into_scaled_division,
//!   test_nbody_address_shifts_combine_without_an_extra_counter,
//!   test_nbody_damping_keeps_negation_whole,
//!   test_nbody_damping_reverses_subtraction_without_negation,
//!   test_algebraic_pass_runs_without_constant_propagation_facts,
//!   test_harr_only_needs_the_low_product,
//!   test_dead_phis_do_not_keep_matrix_product_halves.
//! - monkeypatching `mir.consumed`: test_zero_test_forwarding_indexes_each_operation_once.

use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;

use super::*;
use crate::model::ir::Operation;
use crate::model::mir::{
    Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Symbol, Synth,
    Value,
};
use crate::objectfile::module::{Addr, Space};

fn value(id: u32, at: i64) -> Value {
    Value::new(id, at)
}

fn flag(id: u32, at: i64) -> Value {
    Value {
        flags: true,
        ..Value::new(id, at)
    }
}

fn held(value: Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

fn constant(n: impl Into<BigInt>, width: u32) -> Arg {
    Arg::Const(Const::new(n, width))
}

fn binary(
    at: i64,
    name: &str,
    kind: Kind,
    defines: Vec<Value>,
    uses: Vec<Value>,
    args: Vec<Arg>,
    results: Vec<Arg>,
) -> Op {
    Op {
        kind,
        args,
        results,
        ..Op::new(
            at,
            OpCode::Operation(Operation::Binary),
            name,
            defines,
            uses,
        )
    }
}

fn counter(entries: &[(Value, usize)]) -> BTreeMap<Value, usize> {
    entries.iter().copied().collect()
}

fn set(values: &[Value]) -> BTreeSet<Value> {
    values.iter().copied().collect()
}

fn definitions<'a>(entries: &[(Value, &'a Op)]) -> BTreeMap<Value, &'a Op> {
    entries.iter().copied().collect()
}

fn one_block(ops: Vec<Op>) -> MirBody {
    MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])])
}

fn merges(entries: &[(Value, Value)]) -> OrderedMap<Value, Value> {
    entries.iter().copied().collect()
}

#[test]
fn test_offset_composition_preserves_modular_values_and_observers() {
    for kind in [Kind::Add, Kind::Sub] {
        for guard in [
            "none",
            "first_flags",
            "last_flags",
            "shared",
            "width",
            "merge",
            "memory",
        ] {
            let (source, middle, result) = (value(1, 0), value(2, 0), value(3, 0));
            let flags = flag(4, 0);
            let mut first = binary(
                0,
                "add",
                Kind::Add,
                vec![middle],
                vec![source],
                vec![held(source, 2), constant(65530, 2)],
                vec![held(middle, 2)],
            );
            let mut last = binary(
                1,
                if kind == Kind::Add { "add" } else { "sub" },
                kind,
                vec![result],
                vec![middle],
                vec![held(middle, 2), constant(20, 2)],
                vec![held(result, 2)],
            );
            match guard {
                "first_flags" => first.defines = vec![middle, flags],
                "last_flags" => last.defines = vec![result, flags],
                "width" => last.results = vec![held(result, 4)],
                "merge" => first.merges = merges(&[(source, middle)]),
                "memory" => first.loads = vec![MemRef::new(None, 2)],
                _ => {}
            }
            let uses = counter(&[(middle, if guard == "shared" { 2 } else { 1 })]);
            let done = _offset_chain(
                &last,
                &definitions(&[(middle, &first)]),
                &set(&[flags]),
                &uses,
            ).into_owned();
            if guard != "none" {
                assert_eq!(done, last, "{guard}");
                continue;
            }
            let delta: i64 = if kind == Kind::Add { 20 } else { -20 };
            assert_eq!(
                done.args,
                vec![held(source, 2), constant((65530 + delta) & 65535, 2)]
            );
            let Arg::Const(amount) = &done.args[1] else {
                unreachable!()
            };
            for number in [0_i64, 1, 32767, 32768, 65535] {
                assert_eq!(
                    BigInt::from((((number + 65530) & 65535) + delta) & 65535),
                    (BigInt::from(number) + &amount.n) & BigInt::from(65535)
                );
            }
        }
    }
}

#[test]
fn test_associative_bitwise_constants_combine_without_losing_observers() {
    for (kind, first, last, combined) in [
        (Kind::And, 0xF0F3_i64, 0x3FFF_i64, 0x30F3_i64),
        (Kind::Or, 0xF003, 0x0F30, 0xFF33),
        (Kind::Xor, 0xFFFF, 0x0031, 0xFFCE),
    ] {
        for guard in [
            "none",
            "first_flags",
            "last_flags",
            "shared",
            "width",
            "merge",
            "memory",
        ] {
            let (source, middle, result) = (value(1, 0), value(2, 0), value(3, 0));
            let flags = flag(4, 0);
            let name = kind.to_string();
            let mut first_op = binary(
                0,
                &name,
                kind,
                vec![middle],
                vec![source],
                vec![held(source, 2), constant(first, 2)],
                vec![held(middle, 2)],
            );
            let mut last_op = binary(
                1,
                &name,
                kind,
                vec![result],
                vec![middle],
                vec![held(middle, 2), constant(last, 2)],
                vec![held(result, 2)],
            );
            match guard {
                "first_flags" => first_op.defines = vec![middle, flags],
                "last_flags" => last_op.defines = vec![result, flags],
                "width" => last_op.results = vec![held(result, 4)],
                "merge" => last_op.merges = merges(&[(middle, result)]),
                "memory" => first_op.loads = vec![MemRef::new(None, 2)],
                _ => {}
            }
            let uses = counter(&[(middle, if guard == "shared" { 2 } else { 1 })]);
            let changed = _bitwise_chain(
                &last_op,
                &definitions(&[(middle, &first_op)]),
                &set(&[flags]),
                &uses,
            ).into_owned();
            if !matches!(guard, "none" | "last_flags") {
                assert_eq!(changed, last_op, "{guard}");
                continue;
            }
            assert_eq!(changed.args, vec![held(source, 2), constant(combined, 2)]);
            assert_eq!(changed.defines, last_op.defines);
            for number in [0_i64, 1, 0x7FFF, 0x8000, 0xFFFF] {
                let (expected, actual) = match kind {
                    Kind::And => ((number & first) & last, number & combined),
                    Kind::Or => ((number | first) | last, number | combined),
                    _ => ((number ^ first) ^ last, number ^ combined),
                };
                assert_eq!(expected, actual);
            }
        }
    }
}

#[test]
fn test_scaled_chain_preserves_modular_values_and_observed_intermediates() {
    for guard in [
        "none",
        "first_flags",
        "last_flags",
        "shared",
        "width",
        "merge",
    ] {
        let (source, middle, result) = (value(1, 0), value(2, 0), value(3, 0));
        let flags = flag(4, 0);
        let mut first = binary(
            0,
            "mul",
            Kind::Mul,
            vec![middle],
            vec![source],
            vec![held(source, 2), constant(32769, 2)],
            vec![held(middle, 2)],
        );
        let mut last = binary(
            1,
            "shl",
            Kind::Shl,
            vec![result],
            vec![middle],
            vec![held(middle, 2), constant(1, 1)],
            vec![held(result, 2)],
        );
        match guard {
            "first_flags" => first.defines = vec![middle, flags],
            "last_flags" => last.defines = vec![result, flags],
            "width" => last.results = vec![held(result, 4)],
            "merge" => last.merges = merges(&[(source, result)]),
            _ => {}
        }
        let uses = counter(&[(middle, if guard == "shared" { 2 } else { 1 })]);
        let done = _scaled_chain(
            &last,
            &definitions(&[(middle, &first)]),
            &set(&[flags]),
            &uses,
        ).into_owned();
        if guard != "none" {
            assert_eq!(done, last, "{guard}");
            continue;
        }
        assert_eq!(done.kind, Kind::Mul);
        assert_eq!(done.args, vec![held(source, 2), constant(2, 2)]);
        for number in [0_i64, 1, 32767, 32768, 65535] {
            assert_eq!(((number * 32769 & 65535) << 1) & 65535, number * 2 & 65535);
        }
    }
}

#[test]
fn test_shared_shift_requires_available_same_width_value() {
    for guard in ["none", "unused", "flags", "width", "block"] {
        let (source, middle, result, observed) =
            (value(1, 0), value(2, 0), value(3, 0), value(4, 0));
        let flags = flag(5, 0);
        let first = binary(
            0,
            "shl",
            Kind::Shl,
            vec![middle],
            vec![source],
            vec![held(source, 2), constant(1, 1)],
            vec![held(middle, 2)],
        );
        let observe = Op {
            kind: Kind::Copy,
            args: vec![held(middle, 2)],
            results: vec![held(observed, 2)],
            ..Op::new(
                1,
                OpCode::Operation(Operation::Move),
                "mov",
                vec![observed],
                vec![middle],
            )
        };
        let width = if guard == "width" { 4 } else { 2 };
        let mut last = binary(
            2,
            "shl",
            Kind::Shl,
            vec![result],
            vec![source],
            vec![held(source, width), constant(2, 1)],
            vec![held(result, width)],
        );
        if guard == "flags" {
            last.defines = vec![result, flags];
        }
        let prefix = if guard == "unused" {
            vec![first]
        } else {
            vec![first, observe]
        };
        let blocks = if guard == "block" {
            vec![
                MirBlock::new(0, vec![], prefix, vec![2]),
                MirBlock::new(2, vec![], vec![last.clone()], vec![]),
            ]
        } else {
            vec![MirBlock::new(
                0,
                vec![],
                prefix.into_iter().chain([last.clone()]).collect(),
                vec![],
            )]
        };
        let shared = _shared_shifts(&MirBody::new(0, blocks), &set(&[flags]));
        let done = shared.blocks.last().unwrap().ops.last().unwrap();
        if guard != "none" {
            assert_eq!(*done, last, "{guard}");
            continue;
        }
        assert_eq!(done.args, vec![held(middle, 2), constant(1, 1)]);
        for number in [0_i64, 1, 16383, 16384, 32767, 32768, 65535] {
            assert_eq!(
                (((number << 1) & 65535) << 1) & 65535,
                (number << 2) & 65535
            );
        }
    }
}

#[test]
fn test_shared_shift_distinguishes_a_word_tie_from_a_partial_write() {
    for partial in [false, true] {
        let (source, middle, result) = (value(1, 0), value(2, 0), value(3, 0));
        let width = if partial { 1 } else { 2 };
        let first_result = held(middle, width);
        let final_result = held(result, width);
        let first = Op {
            merges: merges(&[(source, middle)]),
            ..binary(
                0,
                "shl",
                Kind::Shl,
                vec![middle],
                vec![source],
                vec![held(source, width), constant(1, 1)],
                vec![first_result.clone()],
            )
        };
        let last = Op {
            merges: merges(&[(source, result)]),
            ..binary(
                1,
                "shl",
                Kind::Shl,
                vec![result],
                vec![source],
                vec![held(source, width), constant(2, 1)],
                vec![final_result],
            )
        };
        let observe = Op {
            kind: Kind::Opaque,
            args: vec![first_result.clone()],
            ..Op::new(
                2,
                OpCode::Operation(Operation::Move),
                "mov",
                vec![],
                vec![middle],
            )
        };

        let shared = _shared_shifts(
            &one_block(vec![first, observe, last.clone()]),
            &BTreeSet::new(),
        );
        let done = shared.blocks[0].ops.last().unwrap();

        if partial {
            assert_eq!(*done, last);
        } else {
            assert_eq!(done.args, vec![first_result, constant(1, 1)]);
            assert_eq!(done.uses, vec![middle]);
            assert_eq!(done.merges, merges(&[(middle, result)]));
        }
    }
}

#[test]
fn test_extracted_halves_recombine_to_the_original_value() {
    for (high_offset, different_source, recombined) in
        [(16, false, true), (0, false, false), (16, true, false)]
    {
        let (source, other, low, high, result) = (
            value(1, 0),
            value(2, 0),
            value(3, 0),
            value(4, 0),
            value(5, 0),
        );
        let extract = |value: Value, original: Value, offset: i64| Op {
            kind: Kind::Extract,
            args: vec![held(original, 4), constant(offset, 4)],
            results: vec![held(value, 2)],
            ..Op::new(
                0,
                OpCode::Synth(Synth::HalfToLow),
                "extract",
                vec![value],
                vec![original],
            )
        };
        let concat = Op {
            kind: Kind::Concat,
            args: vec![held(high, 2), held(low, 2)],
            results: vec![held(result, 4)],
            ..Op::new(
                1,
                OpCode::Synth(Synth::ConcatLow),
                "concat",
                vec![result],
                vec![high, low],
            )
        };
        let body = one_block(vec![
            extract(low, source, 0),
            extract(
                high,
                if different_source { other } else { source },
                high_offset,
            ),
            concat.clone(),
        ]);
        let done = simplified(&Rc::new(MirBody::clone(&body)), &set(&[result]), &BTreeSet::new()).unwrap();
        let done = done.blocks[0].ops.last().unwrap();
        if recombined {
            assert_eq!(done.kind, Kind::Copy);
            assert_eq!(done.args, vec![held(source, 4)]);
        } else {
            assert_eq!(*done, concat);
        }
    }
}

#[test]
fn test_joined_halves_are_consumed_as_halves() {
    for mode in ["halves", "ordered", "nonzero", "whole_use"] {
        let (low, high, whole, other) = (value(1, 0), value(2, 0), value(3, 0), value(5, 0));
        let flags = flag(4, 0);
        let mut address = Addr::new(Space::Segment, 8);
        address.index = 5;
        let r#ref = MemRef {
            space: Some(Space::Segment),
            ..MemRef::new(Some(address), 4)
        };
        let nothing = |at: i64, defines: Vec<Value>, uses: Vec<Value>| {
            Op::new(at, OpCode::Operation(Operation::Nothing), "", defines, uses)
        };
        let mut ops = vec![
            Op {
                kind: Kind::Call,
                results: vec![held(low, 2), held(high, 2)],
                ..nothing(1, vec![low, high], vec![])
            },
            Op {
                kind: Kind::Concat,
                args: vec![held(high, 2), held(low, 2)],
                results: vec![held(whole, 4)],
                ..nothing(2, vec![whole], vec![high, low])
            },
            Op {
                kind: Kind::Store,
                args: vec![held(whole, 4)],
                results: vec![Arg::Cell(Cell {
                    r#ref: r#ref.clone(),
                })],
                stores: vec![r#ref.clone()],
                ..nothing(3, vec![], vec![whole])
            },
            Op {
                kind: Kind::Arg,
                args: vec![held(whole, 4)],
                ..nothing(4, vec![], vec![whole])
            },
            Op {
                kind: Kind::Sub,
                args: vec![
                    held(whole, 4),
                    constant(if mode == "nonzero" { 5 } else { 0 }, 4),
                ],
                ..nothing(5, vec![flags], vec![whole])
            },
            Op {
                kind: Kind::Branch,
                test: Some(if mode == "ordered" {
                    Kind::Lt
                } else {
                    Kind::Ne
                }),
                target: Some(0),
                ..nothing(6, vec![], vec![flags])
            },
        ];
        if mode == "whole_use" {
            ops.insert(
                5,
                Op {
                    kind: Kind::Shr,
                    args: vec![held(whole, 4), constant(1, 1)],
                    results: vec![held(other, 4)],
                    ..nothing(5, vec![other], vec![whole])
                },
            );
        }
        let count = ops.len();
        let done = simplified(&Rc::new(MirBody::clone(&one_block(ops))), &set(&[other]), &BTreeSet::new()).unwrap();
        let done = &done.blocks[0].ops;
        let readers = done.iter().filter(|op| op.uses.contains(&whole)).count();
        if mode != "halves" {
            assert_eq!(readers, count - 3, "{mode}");
            continue;
        }
        assert_eq!(readers, 0);
        let stores = done
            .iter()
            .filter(|op| op.kind == Kind::Store)
            .map(|op| (op.args.clone(), op.stores[0].addr, op.stores[0].width))
            .collect::<Vec<_>>();
        assert_eq!(
            stores,
            vec![
                (vec![held(low, 2)], r#ref.addr, 2),
                (vec![held(high, 2)], r#ref.addr.map(|addr| addr.plus(2)), 2)
            ]
        );
        assert_eq!(
            done.iter()
                .filter(|op| op.kind == Kind::Arg)
                .map(|op| op.args.clone())
                .collect::<Vec<_>>(),
            vec![vec![held(high, 2)], vec![held(low, 2)]]
        );
        let test = done.iter().find(|op| op.defines.contains(&flags)).unwrap();
        assert_eq!(test.kind, Kind::Or);
        assert_eq!(test.args.len(), 2);
        assert!(test.args.contains(&held(high, 2)) && test.args.contains(&held(low, 2)));
    }
}

#[test]
fn test_recombination_follows_only_exact_word_copies() {
    for mode in ["copy", "width_change", "cycle"] {
        let (source, high, low, copied) = (value(1, 0), value(2, 0), value(3, 0), value(4, 0));
        let mut ops = Vec::new();
        for (value, offset) in [(high, 16), (low, 0)] {
            ops.push(Op {
                kind: Kind::Extract,
                args: vec![held(source, 4), constant(offset, 4)],
                results: vec![held(value, 2)],
                ..Op::new(
                    0,
                    OpCode::Synth(Synth::HalfToLow),
                    "extract",
                    vec![value],
                    vec![source],
                )
            });
        }
        let incoming = if mode == "cycle" { copied } else { low };
        ops.push(Op {
            kind: Kind::Copy,
            args: vec![held(incoming, if mode == "width_change" { 4 } else { 2 })],
            results: vec![held(copied, 2)],
            ..Op::new(
                1,
                OpCode::Operation(Operation::Move),
                "mov",
                vec![copied],
                vec![incoming],
            )
        });
        let found = definitions(&[(high, &ops[0]), (low, &ops[1]), (copied, &ops[2])]);
        let answer = mir::extracted_whole(&held(high, 2), &held(copied, 2), &found);
        assert_eq!(
            answer,
            if mode == "copy" {
                Some(Held {
                    value: source,
                    width: 4,
                })
            } else {
                None
            },
            "{mode}"
        );
    }
}

#[test]
fn test_signed_recombination_compares_copy_sources_symmetrically() {
    for mode in ["same", "sibling", "different", "width_change", "barrier"] {
        let (root, copied, sibling, whole, high) = (
            value(1, 0),
            value(2, 0),
            value(3, 0),
            value(4, 0),
            value(5, 0),
        );
        let copy = |value: Value| Op {
            kind: Kind::Copy,
            args: vec![held(root, 2)],
            results: vec![held(value, 2)],
            ..Op::new(
                0,
                OpCode::Operation(Operation::Move),
                "mov",
                vec![value],
                vec![root],
            )
        };
        let mut copied_op = copy(copied);
        let sibling_op = copy(sibling);
        if mode == "width_change" {
            copied_op.args = vec![held(root, 4)];
        }
        if mode == "barrier" {
            copied_op.op = Some(OpCode::Operation(Operation::Barrier));
        }
        let extension = Op {
            kind: Kind::SignExtend,
            args: vec![held(copied, 2)],
            results: vec![held(whole, 4)],
            ..Op::new(
                1,
                OpCode::Operation(Operation::Extend),
                "sign_extend",
                vec![whole],
                vec![copied],
            )
        };
        let extract = Op {
            kind: Kind::Extract,
            args: vec![held(whole, 4), constant(16, 4)],
            results: vec![held(high, 2)],
            ..Op::new(
                1,
                OpCode::Synth(Synth::HalfToLow),
                "extract",
                vec![high],
                vec![whole],
            )
        };
        let found = definitions(&[
            (copied, &copied_op),
            (sibling, &sibling_op),
            (whole, &extension),
            (high, &extract),
        ]);
        let low = match mode {
            "same" => copied,
            "different" => value(99, 0),
            _ => sibling,
        };
        let answer = mir::extracted_whole(&held(high, 2), &held(low, 2), &found);
        let expected = matches!(mode, "same" | "sibling").then_some(Held {
            value: whole,
            width: 4,
        });
        assert_eq!(answer, expected, "{mode}");
    }
}

#[test]
fn test_reversed_difference_preserves_observed_values_and_flags() {
    for guard in ["none", "shared", "sub_flags", "neg_flags", "width", "merge"] {
        let (left, right, middle, result) = (value(1, 0), value(2, 0), value(3, 0), value(4, 0));
        let flags = flag(5, 0);
        let mut difference = binary(
            0,
            "sub",
            Kind::Sub,
            vec![middle],
            vec![left, right],
            vec![held(left, 4), held(right, 4)],
            vec![held(middle, 4)],
        );
        let mut negate = Op {
            kind: Kind::Neg,
            args: vec![held(middle, 4)],
            results: vec![held(result, 4)],
            ..Op::new(
                1,
                OpCode::Operation(Operation::Unary),
                "neg",
                vec![result],
                vec![middle],
            )
        };
        match guard {
            "sub_flags" => difference.defines = vec![middle, flags],
            "neg_flags" => negate.defines = vec![result, flags],
            "width" => negate.results = vec![held(result, 2)],
            "merge" => difference.merges = merges(&[(left, middle)]),
            _ => {}
        }
        let uses = counter(&[(middle, if guard == "shared" { 2 } else { 1 })]);
        let done = _negated_difference(
            &negate,
            &definitions(&[(middle, &difference)]),
            &set(&[flags]),
            &uses,
        ).into_owned();
        if guard != "none" {
            assert_eq!(done, negate, "{guard}");
            continue;
        }
        assert_eq!(done.kind, Kind::Sub);
        assert_eq!(
            done.args,
            difference.args.iter().rev().cloned().collect::<Vec<_>>()
        );
        for first in [0_i64, 1, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF] {
            for second in [0_i64, 1, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF] {
                assert_eq!(
                    (-((first - second) & 0xFFFF_FFFF)) & 0xFFFF_FFFF,
                    (second - first) & 0xFFFF_FFFF
                );
            }
        }
    }
}

#[test]
fn test_shift_combination_preserves_count_flag_and_use_boundaries() {
    for (first_count, last_count, live_flags, uses) in [
        (15, 1, false, 1),
        (32, 1, false, 1),
        (1, 1, true, 1),
        (1, 1, false, 2),
    ] {
        let (source, middle, result) = (value(1, 0), value(2, 0), value(3, 0));
        let flags = flag(4, 0);
        let first = binary(
            0,
            "shl",
            Kind::Shl,
            vec![middle],
            vec![source],
            vec![held(source, 2), constant(first_count, 1)],
            vec![held(middle, 2)],
        );
        let last = binary(
            1,
            "shl",
            Kind::Shl,
            vec![result, flags],
            vec![middle],
            vec![held(middle, 2), constant(last_count, 1)],
            vec![held(result, 2)],
        );
        let wanted = if live_flags {
            set(&[flags])
        } else {
            BTreeSet::new()
        };
        assert_eq!(
            _shift_chain(
                &last,
                &definitions(&[(middle, &first)]),
                &wanted,
                &counter(&[(middle, uses)])
            ).into_owned(),
            last
        );
    }
}

#[test]
fn test_signed_power_division_preserves_quotient_and_remainder() {
    for (width, divisor) in [
        (4_u32, 2_i64),
        (4, 16),
        (4, 512),
        (4, 262144),
        (2, 2),
        (2, 16),
        (2, 16384),
    ] {
        for immediate in [false, true] {
            let bits = 8 * width;
            let (source, constant_value, quotient, remainder) =
                (value(1, 0), value(2, 0), value(3, 0), value(4, 0));
            let copy = Op {
                kind: Kind::Copy,
                args: vec![constant(divisor, width)],
                results: vec![held(constant_value, width)],
                ..Op::new(
                    0,
                    OpCode::Operation(Operation::Move),
                    "mov",
                    vec![constant_value],
                    vec![],
                )
            };
            let mut divide = Op {
                kind: Kind::Divmod,
                args: vec![held(source, width), held(constant_value, width)],
                results: vec![held(quotient, width), held(remainder, width)],
                ..Op::new(
                    1,
                    OpCode::Operation(Operation::Divide),
                    "idiv",
                    vec![quotient, remainder],
                    vec![source, constant_value],
                )
            };
            let body = if immediate {
                divide.args = vec![held(source, width), constant(divisor, width)];
                divide.uses = vec![source];
                one_block(vec![divide])
            } else {
                one_block(vec![copy, divide])
            };
            let done = _divisions(&Rc::new(MirBody::clone(&body)));
            assert!(done.blocks[0].ops.iter().all(|op| op.kind != Kind::Divmod));
            let (lowest, highest) = (-(1_i64 << (bits - 1)), (1_i64 << (bits - 1)) - 1);
            for number in [
                lowest,
                -divisor - 1,
                -divisor,
                -divisor + 1,
                -1,
                0,
                1,
                divisor - 1,
                divisor,
                highest,
            ] {
                let mut values = BTreeMap::from([(source, number)]);
                for op in &done.blocks[0].ops {
                    let args = op
                        .args
                        .iter()
                        .map(|arg| match arg {
                            Arg::Const(one) => i64::try_from(&one.n).unwrap(),
                            Arg::Held(one) => values[&one.value],
                            _ => unreachable!(),
                        })
                        .collect::<Vec<_>>();
                    let answer = match op.kind {
                        Kind::Copy => args[0],
                        Kind::Sar => args[0] >> args[1],
                        Kind::Shr => (args[0] & ((1_i64 << bits) - 1)) >> args[1],
                        Kind::Shl => args[0] << args[1],
                        Kind::And => args[0] & args[1],
                        Kind::Add => args[0] + args[1],
                        Kind::Sub => args[0] - args[1],
                        other => panic!("{other}"),
                    };
                    let Arg::Held(result) = op.results[0] else {
                        unreachable!()
                    };
                    values.insert(
                        result.value,
                        ((answer & ((1_i64 << bits) - 1)) ^ (1_i64 << (bits - 1)))
                            - (1_i64 << (bits - 1)),
                    );
                }
                let expected = number.abs() / divisor * if number < 0 { -1 } else { 1 };
                assert_eq!(values[&quotient], expected);
                assert_eq!(values[&remainder], number - expected * divisor);
            }
        }
    }
}

#[test]
fn test_constant_word_concatenation() {
    for (high, low, answer) in [
        (4_i64, 0_i64, 262144_i64),
        (0, 512, 512),
        (-1, -1, 0xFFFF_FFFF),
        (1, -1, 0x1FFFF),
    ] {
        let result = value(1, 0);
        let op = Op {
            kind: Kind::Concat,
            args: vec![constant(high, 2), constant(low, 2)],
            results: vec![held(result, 4)],
            ..Op::new(
                0,
                OpCode::Synth(Synth::ConcatLow),
                "concat",
                vec![result],
                vec![],
            )
        };
        let done = simplified(&Rc::new(MirBody::clone(&one_block(vec![op]))), &set(&[result]), &BTreeSet::new()).unwrap();
        let done = &done.blocks[0].ops[0];
        assert_eq!(done.kind, Kind::Copy);
        assert_eq!(done.args, vec![constant(answer, 4)]);
    }
}

#[test]
fn test_integer_identities() {
    for width in [2_u32, 4] {
        for (kind, number, answer) in [
            (Kind::Add, 0_i64, None),
            (Kind::Sub, 0, None),
            (Kind::Mul, 1, None),
            (Kind::Or, 0, None),
            (Kind::Xor, 0, None),
            (Kind::And, -1, None),
            (Kind::And, 0, Some(0_i64)),
            (Kind::Mul, 0, Some(0)),
            (Kind::Or, -1, Some(-1)),
            (Kind::Shl, 0, None),
            (Kind::Shr, 0, None),
            (Kind::Sar, 0, None),
        ] {
            let (source, result) = (value(10, 0), value(11, 1));
            let count_width = if matches!(kind, Kind::Shl | Kind::Shr | Kind::Sar) {
                1
            } else {
                width
            };
            let op = binary(
                1,
                "",
                kind,
                vec![result],
                vec![source],
                vec![held(source, width), constant(number, count_width)],
                vec![held(result, width)],
            );
            let changed = _simplified(&op, &set(&[result]), &BTreeSet::new()).into_owned();
            assert_eq!(changed.kind, Kind::Copy);
            let expected = match answer {
                None => held(source, width),
                Some(answer) => constant(
                    BigInt::from(answer) & ((BigInt::from(1) << (width * 8)) - 1),
                    width,
                ),
            };
            assert_eq!(changed.args, vec![expected]);
            assert_eq!(changed.defines, vec![result]);
            if width == 2 {
                assert_eq!(_simplified(&op, &set(&[result]), &set(&[result])).into_owned(), op);
            }
            let flags = flag(12, 1);
            let observed_flags = Op {
                defines: vec![result, flags],
                ..op.clone()
            };
            assert_eq!(
                _simplified(&observed_flags, &set(&[result, flags]), &BTreeSet::new()).into_owned(),
                observed_flags
            );
        }
    }
}

#[test]
fn test_product_projection_retains_observed_outputs() {
    for observed in ["high", "flags", "upper", "none"] {
        let (source, low, high, flags) = (value(1, 0), value(2, 1), value(3, 1), flag(4, 1));
        let op = Op {
            kind: Kind::Mul,
            args: vec![held(source, 2), constant(20, 2)],
            results: vec![held(low, 2), held(high, 2)],
            merges: merges(&[(source, high)]),
            ..Op::new(
                1,
                OpCode::Operation(Operation::Multiply),
                "imul",
                vec![flags, low, high],
                vec![source],
            )
        };
        let mut wanted = set(&[low]);
        match observed {
            "high" => {
                wanted.insert(high);
            }
            "flags" => {
                wanted.insert(flags);
            }
            _ => {}
        }
        let wide = if observed == "upper" {
            set(&[low])
        } else {
            BTreeSet::new()
        };
        let changed = _product(&op, &wanted, &wide).into_owned();
        if observed == "none" {
            assert_eq!(changed.results, vec![held(low, 2)]);
            assert_eq!(changed.defines, vec![low]);
            assert_eq!(changed.uses, vec![source]);
        } else {
            assert_eq!(changed, op, "{observed}");
        }
    }
}

#[test]
fn test_a_symbol_plus_zero_is_the_symbol() {
    let symbol = Symbol::new(Space::Segment, 2, 0, 2);
    let result = value(1, 0);
    let add = binary(
        0,
        "add",
        Kind::Add,
        vec![result],
        vec![],
        vec![constant(0, 2), Arg::Symbol(symbol)],
        vec![held(result, 2)],
    );
    let done = _simplified(&add, &BTreeSet::new(), &BTreeSet::new()).into_owned();
    assert_eq!(
        (done.kind, done.args),
        (Kind::Copy, vec![Arg::Symbol(symbol)])
    );
}

fn extension(at: i64, result: Value, source: Value, source_width: u32) -> Op {
    Op {
        kind: Kind::ZeroExtend,
        args: vec![held(source, source_width)],
        results: vec![held(result, 2)],
        ..Op::new(
            at,
            OpCode::Operation(Operation::Extend),
            "",
            vec![result],
            vec![source],
        )
    }
}

#[test]
fn test_reextending_an_already_zero_extended_low_byte_is_a_copy() {
    let (source, middle, result) = (value(1, 0), value(2, 0), value(3, 0));
    let first = extension(1, middle, source, 1);
    let second = extension(2, result, middle, 1);
    let used = Op {
        kind: Kind::Arg,
        args: vec![held(result, 1)],
        ..Op::new(
            3,
            OpCode::Operation(Operation::Push),
            "",
            vec![],
            vec![result],
        )
    };

    let done = simplified(
        &Rc::new(MirBody::clone(&one_block(vec![first, second, used]))),
        &BTreeSet::new(),
        &BTreeSet::new(),
    )
    .unwrap();
    let changed = &done.blocks[0].ops[1];

    assert_eq!(changed.kind, Kind::Copy);
    assert_eq!(changed.args, vec![held(middle, 2)]);
}

#[test]
fn test_redundant_extension_requires_every_output_bit_to_be_known() {
    for guard in [
        "signedness",
        "discarded_bits",
        "unknown_upper",
        "extra_result",
    ] {
        let (source, middle, result, flags) = (value(1, 0), value(2, 0), value(3, 0), flag(4, 0));
        let mut first = extension(1, middle, source, 1);
        let mut second = extension(2, result, middle, 1);
        match guard {
            "signedness" => second.kind = Kind::SignExtend,
            "discarded_bits" => first.args = vec![held(source, 2)],
            "unknown_upper" => second.results = vec![held(result, 4)],
            _ => second.defines = vec![result, flags],
        }
        assert_eq!(
            _redundant_extension(&second, &definitions(&[(middle, &first)])).into_owned(),
            second,
            "{guard}"
        );
    }
}

fn zero_copy(zero: Value) -> Op {
    Op {
        kind: Kind::Copy,
        args: vec![constant(0, 4)],
        results: vec![held(zero, 4)],
        ..Op::new(
            1,
            OpCode::Operation(Operation::Move),
            "",
            vec![zero],
            vec![],
        )
    }
}

#[test]
fn test_subtracting_from_a_copied_zero_is_negation() {
    let (zero, source, result, flags) = (value(1, 0), value(2, 0), value(3, 0), flag(4, 0));
    let constant_op = zero_copy(zero);
    let subtract = binary(
        2,
        "",
        Kind::Sub,
        vec![result, flags],
        vec![zero, source],
        vec![held(zero, 4), held(source, 4)],
        vec![held(result, 4)],
    );

    let changed = _zero_difference(&subtract, &definitions(&[(zero, &constant_op)])).into_owned();

    assert_eq!(changed.kind, Kind::Neg);
    assert_eq!(changed.args, vec![held(source, 4)]);
    assert_eq!(changed.defines, vec![result, flags]);
    assert_eq!(changed.uses, vec![source]);
}

#[test]
fn test_zero_difference_requires_a_complete_pure_value() {
    for guard in [
        "nonzero",
        "width",
        "effect",
        "extra_result",
        "untracked_use",
        "cycle",
    ] {
        let (zero, source, result, extra) = (value(1, 0), value(2, 0), value(3, 0), value(4, 0));
        let mut constant_op = zero_copy(zero);
        let mut subtract = binary(
            2,
            "",
            Kind::Sub,
            vec![result],
            vec![zero, source],
            vec![held(zero, 4), held(source, 4)],
            vec![held(result, 4)],
        );
        match guard {
            "nonzero" => constant_op.args = vec![constant(1, 4)],
            "width" => subtract.results = vec![held(result, 2)],
            "effect" => constant_op.op = Some(OpCode::Operation(Operation::Barrier)),
            "extra_result" => subtract.defines = vec![result, extra],
            "untracked_use" => subtract.uses = vec![source],
            _ => {
                constant_op.args = vec![held(zero, 4)];
                constant_op.uses = vec![zero];
            }
        }
        assert_eq!(
            _zero_difference(&subtract, &definitions(&[(zero, &constant_op)])).into_owned(),
            subtract,
            "{guard}"
        );
    }
}
