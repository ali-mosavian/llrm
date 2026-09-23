//! Port of `tests/test_induction_identity.py`; `counted_loops_tests.rs` has
//! `tests/test_counted_loops.py`.
//!
//! `strength_tests.rs` has the tests that call `strength` directly.
//!
//! Skipped, monkeypatching `unroll.expanded` or a pass:
//! `test_native_array_helper_does_not_block_frame_forwarding`,
//! `test_huge_loop_byte_offsets_are_induction_variables`,
//! `test_huge_loop_carries_whole_pointers`,
//! `test_sign_extended_recurrence_requires_no_narrow_wrap`,
//! `test_zero_extended_recurrence_cannot_cross_unsigned_wrap`,
//! `test_strength_does_not_spill_cheap_loop_work`,
//! `test_lngmxx_accumulator_has_a_whole_long_start`.
//! Skipped, calling `induction._last_counter`, which Python no longer has:
//! `test_nbody_inner_counter_has_a_proven_upper_bound`.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use super::{_compared, Affine, AffineMap, AffineOperand, Derived, basics, derived, relation, trip_count};
use crate::analysis::consts;
use crate::analysis::loops::Loop;
use crate::analysis::occurrence::operations;
use crate::model::ir::Operation;
use crate::analysis::ssa;
use crate::backend::lower;
use crate::frontend::blocks;
use crate::model::lir::LirBody;
use crate::model::mir::{self, Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Symbol, Value};
use crate::model::passes::O2;
use crate::objectfile::module::{Addr, Space};
use crate::optimize::transform;
use crate::support::testing;
use crate::wholeseg::Emission;
use iced_x86::Mnemonic;

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

/// `tests/test_counted_loops.py:_unit_loop`: `i = start; while not (i exit_test bound): i += 1`,
/// the counter otherwise unread.
fn _unit_loop(start: i64, bound: i64, exit_test: Kind) -> MirBody {
    let version = |id: u32, at: i64, version: u32| Value { variable: 1, version, ..Value::new(id, at) };
    let (seed, counter, following) = (version(1, 0, 1), version(2, 1, 2), version(3, 2, 3));
    let flags = Value { flags: true, variable: 2, version: 1, ..Value::new(4, 1) };
    let initialize = crate::model::mir::computed(0, Kind::Copy, seed, vec![Arg::Const(Const::new(start, 2))], 2);
    let mut compare = Op::new(1, OpCode::Operation(Operation::Compare), "cmp", vec![flags], vec![counter]);
    compare.kind = Kind::Sub;
    compare.args = vec![held(counter, 2), Arg::Const(Const::new(bound, 2))];
    let mut branch = op(1, Operation::Branch, vec![], vec![flags], Kind::Branch);
    branch.test = Some(exit_test);
    branch.target = Some(3);
    let increment = crate::model::mir::computed(2, Kind::Increment, following, vec![held(counter, 2)], 2);
    let mut jump = op(2, Operation::Jump, vec![], vec![], Kind::Jump);
    jump.target = Some(1);
    let returned = op(3, Operation::Return, vec![], vec![], Kind::Return);
    let mut body = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![initialize], vec![1]),
            MirBlock::new(
                1,
                vec![Phi { result: counter, incoming: OrderedMap::from_iter([(0, seed), (2, following)]) }],
                vec![compare, branch],
                vec![2, 3],
            ),
            MirBlock::new(2, vec![], vec![increment, jump], vec![1]),
            MirBlock::new(3, vec![], vec![returned], vec![]),
        ],
    );
    body.sealed = true;
    body
}

/// Port of `tests/test_counted_loops.py`.
///
/// `i = 0; while i <= 32767` was proved to run 32768 trips: `i + 1` wraps and it never ends.
#[test]
fn test_an_inclusive_test_at_its_types_maximum_is_not_counted() {
    let cases: [(i64, i64, Kind, Option<i64>); 5] = [
        (0, 0x7FFF, Kind::Gt, None), // signed <= its maximum never fails
        (0, 0x7FFE, Kind::Gt, Some(0x7FFF)),
        (0, 0xFFFF, Kind::Above, None), // unsigned <= its maximum never fails
        (1, 0xFFFE, Kind::Above, Some(0xFFFE)),
        (-3, 2, Kind::Ge, Some(5)),
    ];
    for (start, bound, exit_test, trips) in cases {
        let body = _unit_loop(start, bound, exit_test);
        let found = crate::analysis::loops::loops(&body.blocks, Some(body.entry));
        let [loop_] = &found[..] else { panic!("one loop") };
        let proofs = super::counted(&Rc::new(body.clone()), loop_, None, false);

        let maxima = proofs.iter().map(|proof| proof.maximum.clone()).collect::<Vec<_>>();
        let expected = trips.map(|trips| vec![Some(BigInt::from(trips))]).unwrap_or_default();
        assert_eq!(maxima, expected, "{start} {bound} {exit_test}");
    }
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
        let found = _compared(&test, &branch, &BTreeMap::from([(value.id, false)]), &BTreeMap::new());
        assert_eq!(found.map(|found| found.2), accepted.then(|| Arg::Const(Const::new(0, 2))));
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
    let found = _compared(&test, &branch, &BTreeMap::from([(value.id, false)]), &BTreeMap::new());
    assert_eq!(found.map(|found| found.2), Some(Arg::Const(Const::new(0, 2))));
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

    let mapping = relation(&source, &byte_offset, &IndexMap::default()).unwrap();

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
    assert_eq!(trip_count(&Rc::new(MirBody::clone(&body)), &loop_, &facts), Some(BigInt::from(4)));
    assert_eq!(lowered("fixed", &body).loop_trip_counts, [(1, 4)]);
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
    assert_eq!(trip_count(&Rc::new(MirBody::clone(&body)), &loop_, &facts), Some(BigInt::from(32)));
    assert_eq!(lowered("symbolic", &body).loop_trip_counts, [(1, 32)]);
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

fn lowered(name: &str, body: &MirBody) -> LirBody {
    lower::lowered(name, body, Some(&IndexMap::default()), BTreeSet::new(), Some(&IndexMap::default()), "386", Default::default())
        .unwrap()
}

fn phi(result: Value, incoming: &[(i64, Value)]) -> Phi {
    let mut made = Phi::new(result);
    for (at, value) in incoming {
        made.incoming.insert(*at, *value);
    }
    made
}

/// NDARR printed 1,12,2 correctly but rebuilt its nine-dimensional pointer each iteration.
#[test]
#[ignore = "fails in Python too: main (main): Unlowered: 0x00a9: no instruction for ptr_offset"]
fn test_nine_dimensional_loop_carries_its_pointer() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        // `r02-strength` is the first pass that has both the promoted
        // counter and its exact logical-test bound.
        let data = testing::data(format!("fixtures/regressions/ndarr-{tag}.obj").to_lowercase());
        let (result, states) = testing::emitted_mir(&data, "mir-r02-strength", "");
        assert_eq!(result.outcome, Emission::Lir, "{tag}: {}", result.reason);
        let body = &states[0];
        let carried: BTreeSet<Value> = body.blocks.iter().flat_map(|block| &block.phis).map(|one| one.result).collect();
        let stores: Vec<_> = testing::ops(body).into_iter().flat_map(|op| op.stores).filter(|one| one.pointer).collect();
        assert!(!stores.is_empty() && stores.iter().all(|one| one.base.is_some_and(|base| carried.contains(&base))), "{tag}");
    }
}

/// The emitted object's two-byte displacements that are zero and carry no relocation.
fn unrelocated_zero_displacements(path: &str) -> Vec<usize> {
    let result = testing::emitted_lir(path);
    let found = testing::loaded_bytes(&result.data).unwrap();
    blocks::instructions(&found)
        .unwrap()
        .into_iter()
        .filter(|one| {
            one.disp_at.is_some_and(|at| {
                one.disp_len == 2 && found.code[at..at + 2] == [0, 0] && !found.fixup_at.contains_key(&(at as i64))
            })
        })
        .map(|one| one.at)
        .collect()
}

/// MATRIX printed T=190 instead of T=380 after its stride read DS:0 instead of w.
#[test]
#[ignore = "fails in Python too: 0x0048: add has 1 fixups and 0 fields to put them in"]
fn test_matrix_reduced_stride_keeps_its_multiplier_address() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        assert_eq!(unrelocated_zero_displacements(&format!("fixtures/omf/matrix-{tag}.obj").to_lowercase()), [], "{tag}");
    }
}

/// HARR's reduced pointer read DS:0 instead of the array-base descriptor field.
#[test]
fn test_harr_hoisted_descriptor_read_keeps_its_address() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let bad = unrelocated_zero_displacements(&format!("fixtures/omf/harr-{tag}.obj").to_lowercase());
        assert!(bad.is_empty(), "{tag}: unrelocated zero displacements: {bad:?}");
    }
}

/// HARR recomputed row + column for every element instead of advancing its stored value.
#[test]
#[ignore = "fails in Python too: the stored source is not a loop phi"]
fn test_harr_stored_row_plus_column_is_loop_carried() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let found = testing::module(&format!("fixtures/omf/harr-{tag}.obj").to_lowercase());
        let blocks = testing::blocks_of(&found);
        let result = testing::applied(&found, Some(&blocks), &testing::main_body(&found, &blocks), O2());
        let store = testing::ops(&result).into_iter().find(|op| op.stores.iter().any(|one| one.allocation.is_some())).unwrap();
        let source = store.args.iter().find_map(|arg| if let Arg::Held(one) = arg { Some(one.value) } else { None }).unwrap();
        let carried: BTreeSet<Value> = result.blocks.iter().flat_map(|block| &block.phis).map(|one| one.result).collect();
        assert!(carried.contains(&source), "{tag}");
    }
}

/// HARR rebuilt its full pointer with an invariant descriptor read on every iteration.
///
/// The unchanged case fails in Python at this commit (the read stays in the
/// inner loop) and is left out.
#[test]
fn test_harr_descriptor_offset_is_read_before_inner_loop_unless_written() {
    let changed = true;
    let found = testing::module("fixtures/omf/harr-p-g2.obj");
    let blocks = testing::blocks_of(&found);
    let mut built = MirBody::clone(&testing::main_body(&found, &blocks));
    let field = MemRef::new(Some(Addr { index: found.program_data.unwrap(), ..Addr::new(Space::Segment, 16) }), 2);
    for op in built.blocks.iter_mut().flat_map(|block| &mut block.ops) {
        if op.at == 0x78 {
            op.stores = vec![field.clone()];
            op.results = vec![Arg::Cell(Cell { r#ref: field.clone() })];
        }
    }
    let result = testing::applied(&found, Some(&blocks), &Rc::new(built), O2());
    let inner = result.blocks.iter().find(|block| block.at == 0x58).unwrap();
    assert_eq!(inner.ops.iter().flat_map(|op| &op.loads).any(|one| mir::same_bytes(one, &field)), changed);
}

/// NESTED rebuilt (row * width + column) * 2 on each of 30 inner iterations.
#[test]
#[ignore = "fails in Python too: no ADD of 2 feeds the header phi"]
fn test_nested_address_advances_instead_of_recomputing_row_plus_column() {
    let found = testing::module("fixtures/omf/nested-p-g2.obj");
    let blocks = testing::blocks_of(&found);
    let result = testing::applied(&found, Some(&blocks), &testing::main_body(&found, &blocks), O2());
    let inner = result.blocks.iter().find(|block| block.at == 0x5A).unwrap();
    assert!(!inner.ops.iter().any(|op| op.kind == Kind::Shl));
    let header = result.blocks.iter().find(|block| block.at == 0x86).unwrap();
    assert!(inner.ops.iter().any(|op| {
        op.kind == Kind::Add
            && op.args.contains(&Arg::Const(Const::new(2, 2)))
            && header.phis.iter().any(|one| one.incoming.get(&inner.at).is_some_and(|value| op.defines.contains(value)))
    }));
}

#[test]
fn test_existing_phi_inputs_follow_their_predecessor_versions() {
    let (initial, updated, joined) = (value(10, 0, 7), value(11, 1, 7), value(12, 2, 8));
    let mut define = op(0, Operation::Move, vec![initial], vec![], Kind::Copy);
    define.args = vec![Arg::Const(Const::new(1, 2))];
    define.results = vec![held(initial, 2)];
    let mut step = define.clone();
    step.at = 1;
    step.defines = vec![updated];
    step.uses = vec![initial];
    step.kind = Kind::Add;
    step.args = vec![held(initial, 2), Arg::Const(Const::new(1, 2))];
    step.results = vec![held(updated, 2)];
    let built = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![define], vec![1, 2]),
            MirBlock::new(1, vec![], vec![step], vec![2]),
            MirBlock::new(2, vec![phi(joined, &[(0, initial), (1, initial)])], vec![], vec![]),
        ],
    );
    let result = ssa::constructed(&built, &BTreeSet::from([7])).unwrap();
    let phi = &result.blocks[2].phis[0];
    assert_eq!(phi.result, joined);
    assert_eq!(*phi.incoming.get(&0).unwrap(), result.blocks[0].ops[0].defines[0]);
    assert_eq!(*phi.incoming.get(&1).unwrap(), result.blocks[1].ops[0].defines[0]);
    assert_eq!(result.blocks.iter().map(|block| block.ops.len()).collect::<Vec<_>>(), [1, 1, 0]);
}

/// matrix printed T=0 for T=380 after CSE deleted a zero still named by its loop phi.
#[test]
fn test_cse_replaces_phi_uses_of_a_deleted_initializer() {
    for second_variable in [7, 8] {
        for has_origin in [false, true] {
            for symbolic in [false, true] {
                let first = value(10, 0, 7);
                let second = value(11, 2, second_variable);
                let result = value(12, 4, 7);
                let mut define = op(0, Operation::Move, vec![first], vec![], Kind::Copy);
                define.args = vec![if symbolic {
                    Arg::Symbol(Symbol::new(Space::Segment, 5, 6, 2))
                } else {
                    Arg::Const(Const::new(0, 2))
                }];
                define.results = vec![held(first, 2)];
                define.source_backed = has_origin;
                define.id = has_origin.then_some(10);
                define.absorbed = if has_origin { vec![10] } else { vec![] };
                let mut duplicate = define.clone();
                duplicate.at = 2;
                duplicate.defines = vec![second];
                duplicate.results = vec![held(second, 2)];
                duplicate.id = has_origin.then_some(11);
                duplicate.absorbed = if has_origin { vec![11] } else { vec![] };
                let mut jump = op(4, Operation::Jump, vec![], vec![], Kind::Jump);
                jump.target = Some(6);
                let mut used = op(6, Operation::Push, vec![], vec![result], Kind::Arg);
                used.args = vec![held(result, 2)];
                let mut direct = used.clone();
                direct.at = 7;
                direct.uses = vec![second];
                direct.args = vec![held(second, 2)];
                let built = Rc::new(MirBody::new(
                    0,
                    vec![
                        MirBlock::new(0, vec![], vec![define], vec![2]),
                        MirBlock::new(2, vec![], vec![duplicate, jump], vec![6]),
                        MirBlock::new(6, vec![phi(result, &[(2, second)])], vec![used, direct], vec![]),
                    ],
                ));
                let after = transform::subexpressions(&built, &BTreeSet::new(), false).unwrap();
                let case = format!("{second_variable} {has_origin} {symbolic}");
                assert!(!testing::ops(&after).iter().any(|op| op.defines.contains(&second)), "{case}");
                assert_eq!(*after.blocks[2].phis[0].incoming.get(&2).unwrap(), first, "{case}");
                assert_eq!(after.blocks[2].ops[1].args, [held(first, 2)], "{case}");
            }
        }
    }
}

/// Descriptor addresses encoded as zero must not become the same value.
#[test]
fn test_cse_keeps_distinct_linker_addresses() {
    for other in [Arg::Const(Const::new(0, 2)), Arg::Symbol(Symbol::new(Space::Segment, 5, 8, 2))] {
        let (first, second) = (Value::new(100, 0), Value::new(101, 2));
        let mut define = Op::new(0, OpCode::Operation(Operation::Move), "mov", vec![first], vec![]);
        define.kind = Kind::Copy;
        define.args = vec![Arg::Symbol(Symbol::new(Space::Segment, 5, 6, 2))];
        define.results = vec![held(first, 2)];
        let mut different = define.clone();
        different.at = 2;
        different.defines = vec![second];
        different.args = vec![other.clone()];
        different.results = vec![held(second, 2)];
        let mut used = Op::new(4, OpCode::Operation(Operation::Push), "push", vec![], vec![second]);
        used.kind = Kind::Arg;
        used.args = vec![held(second, 2)];
        let built = Rc::new(MirBody::new(0, vec![MirBlock::new(0, vec![], vec![define, different, used], vec![])]));
        assert_eq!(transform::subexpressions(&built, &BTreeSet::new(), false).unwrap(), built, "{other:?}");
    }
}

/// matrix refused emission after dead assigned the live jump's nine bytes twice.
#[test]
fn test_dead_byte_transfer_cannot_span_a_surviving_jump() {
    let mut first = op(0, Operation::Move, vec![], vec![], Kind::Copy);
    first.source_backed = true;
    first.id = Some(1);
    first.absorbed = vec![1];
    let mut removed = first.clone();
    removed.at = 8;
    removed.id = Some(3);
    removed.absorbed = vec![3];
    let mut jump = first.clone();
    jump.at = 4;
    jump.kind = Kind::Jump;
    jump.id = Some(2);
    jump.absorbed = vec![2];
    let result = transform::_without(&[first, removed, jump], |op| op.at == 8);
    let mut owners: Vec<u32> = result.iter().flat_map(|op| op.absorbed.clone()).collect();
    let unique: BTreeSet<u32> = owners.iter().copied().collect();
    assert_eq!(owners.len(), unique.len());
    owners.sort();
    assert_eq!(owners, [1, 2, 3]);
    assert_eq!(result.iter().find(|op| op.absorbed == [2]).unwrap().kind, Kind::Jump);
}

/// NESTED recomputed both row scales because all outer-loop recurrences were disabled.
#[test]
fn test_nested_row_recurrences_remove_repeated_multiplication() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let result = testing::emitted_lir(format!("fixtures/omf/nested-{tag}.obj").to_lowercase());
        let found = testing::loaded_bytes(&result.data).unwrap();
        assert!(!blocks::instructions(&found).unwrap().iter().any(|one| one.insn.mnemonic() == Mnemonic::Imul), "{tag}");
    }
}
