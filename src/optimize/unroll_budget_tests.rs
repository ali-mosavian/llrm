//! Port of tests/test_unroll_budget.py.
//!
//! Skipped, since they monkeypatch `profit.spill_risk`:
//! `test_sequence_budget_counts_the_source_loop_before_candidate_folding`,
//! `test_negligible_spill_improvement_does_not_bypass_the_sequence_budget`,
//! `test_oversized_specialization_may_reduce_existing_spill_burden`.

use super::*;
use crate::model::ir::Operation;
use crate::model::mir::{Const, Held, OpCode};
use crate::model::passes::OperationCosts;

fn op(at: i64, operation: Operation, name: &str, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
    let mut op = Op::new(at, OpCode::Operation(operation), name, defines, uses);
    op.kind = kind;
    op
}

fn _loop() -> MirBody {
    let add = op(1, Operation::Binary, "add", vec![], vec![], Kind::Add);
    let mut branch = op(1, Operation::Branch, "jne", vec![], vec![], Kind::Branch);
    branch.target = Some(1);
    MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![], vec![1]),
            MirBlock::new(1, vec![], vec![add, branch], vec![1, 2]),
            MirBlock::new(2, vec![], vec![], vec![]),
        ],
    )
}

fn _straight(count: i64) -> MirBody {
    let moves = (0..count)
        .map(|at| op(at, Operation::Move, "mov", vec![], vec![], Kind::Copy))
        .collect();
    MirBody::new(
        0,
        vec![MirBlock::new(0, vec![], moves, vec![2]), MirBlock::new(2, vec![], vec![], vec![])],
    )
}

fn held(value: Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

/// Five cheap operations whose four results overlap at the final use.
fn _pressured() -> MirBody {
    let sources = (1..5).map(|number| Value::new(number, 0)).collect::<Vec<_>>();
    let results = (1..5)
        .map(|number| Value::new(number + 4, i64::from(number)))
        .collect::<Vec<_>>();
    let mut ops = (1..5)
        .zip(sources.iter().zip(results.iter()))
        .map(|(number, (source, result))| {
            let mut one = op(i64::from(number), Operation::Binary, "add", vec![*result], vec![*source], Kind::Add);
            one.args = vec![held(*source, 2), Arg::Const(Const::new(number, 2))];
            one.results = vec![held(*result, 2)];
            one
        })
        .collect::<Vec<_>>();
    let total = Value::new(9, 5);
    let mut consume = op(5, Operation::Binary, "add", vec![total], results.clone(), Kind::Add);
    consume.args = results.iter().map(|value| held(*value, 2)).collect();
    consume.results = vec![held(total, 2)];
    ops.push(consume);
    MirBody::new(
        0,
        vec![MirBlock::new(0, vec![], ops, vec![2]), MirBlock::new(2, vec![], vec![], vec![])],
    )
}

/// Two independent groups which each exceed a two-register capacity.
fn _pressure_waves() -> MirBody {
    let mut ops = Vec::new();
    let mut fresh = 1;
    for wave in 0..2_i64 {
        let values = (0..3)
            .map(|index| Value::new(fresh + index, wave * 10 + i64::from(index)))
            .collect::<Vec<_>>();
        fresh += values.len() as u32;
        for (index, value) in values.iter().enumerate() {
            let mut one = op(value.at, Operation::Binary, "add", vec![*value], vec![], Kind::Add);
            one.args = vec![Arg::Const(Const::new(index, 2)), Arg::Const(Const::new(wave, 2))];
            one.results = vec![held(*value, 2)];
            ops.push(one);
        }
        let mut consume = op(wave * 10 + 4, Operation::Binary, "add", vec![], values.clone(), Kind::Add);
        consume.args = values.iter().map(|value| held(*value, 2)).collect();
        ops.push(consume);
    }
    MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])])
}

/// A spill-prone expanded sequence just above GCC's default ceiling.
fn _large_pressured() -> MirBody {
    let mut body = _pressured();
    let padding = (6..202).map(|at| op(at, Operation::Move, "mov", vec![], vec![], Kind::Copy));
    body.blocks[0].ops.extend(padding);
    body.blocks.truncate(1);
    body
}

/// Three x87 values overlap but consume no integer-register capacity.
fn _floating_pressure() -> MirBody {
    let values = (1..4).map(|index| Value::new(index, i64::from(index))).collect::<Vec<_>>();
    let mut ops = values
        .iter()
        .map(|value| {
            let mut one = op(value.at, Operation::FloatLoad, "fld", vec![*value], vec![], Kind::Fload);
            one.results = vec![held(*value, 10)];
            one
        })
        .collect::<Vec<_>>();
    let mut consume = op(4, Operation::Binary, "fadd", vec![], values.clone(), Kind::Fadd);
    consume.args = values.iter().map(|value| held(*value, 10)).collect();
    ops.push(consume);
    MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])])
}

fn costs(change: impl FnOnce(&mut OperationCosts)) -> OperationCosts {
    let mut costs = OperationCosts::default();
    change(&mut costs);
    costs
}

#[test]
fn test_large_complete_peel_must_erase_its_growth_to_cross_the_profile_budget() {
    let costs = costs(|one| {
        one.add = 1;
        one.branch = 50;
        one.r#move = 1;
    });
    let r#where = Where {
        costs,
        options: crate::model::passes::Options { max_unroll_iterations: 16, ..crate::model::passes::Options::default() },
        ..Where::default()
    };
    let original = _loop();

    assert_eq!(_rejection(&original, &_straight(3), 1, 64, &r#where), Some("iteration-growth"));
    assert_eq!(_rejection(&original, &_straight(1), 1, 64, &r#where), None);
    assert_eq!(_rejection(&original, &_straight(3), 1, 16, &r#where), None);
}

#[test]
fn test_default_complete_peel_budget_refuses_sc_init_sized_growth() {
    let costs = costs(|one| {
        one.add = 1;
        one.branch = 50;
        one.r#move = 1;
    });
    let r#where = Where {
        costs,
        ..Where::default()
    };

    assert_eq!(_rejection(&_loop(), &_straight(25), 1, 25, &r#where), Some("iteration-growth"));
    let unbounded = Where {
        options: crate::model::passes::Options { max_unroll_iterations: 0, max_unrolled_operations: 0, ..crate::model::passes::Options::default() },
        ..r#where
    };
    assert_eq!(_rejection(&_loop(), &_straight(25), 1, 25, &unbounded), None);
}

#[test]
fn test_bounded_complete_peel_amortizes_growth_over_its_exact_trip_count() {
    let costs = costs(|one| {
        one.add = 2;
        one.branch = 7;
        one.r#move = 2;
    });
    let r#where = Where {
        costs,
        registers: 6,
        options: crate::model::passes::Options { max_unroll_iterations: 16, ..crate::model::passes::Options::default() },
        ..Where::default()
    };

    assert_eq!(_rejection(&_loop(), &_straight(25), 1, 9, &r#where), None);
}

#[test]
fn test_bounded_peel_with_spill_risk_pays_its_complete_growth() {
    let costs = costs(|one| {
        one.add = 1;
        one.branch = 5;
        one.r#move = 2;
        one.load = 1;
        one.store = 1;
    });
    let r#where = Where {
        costs: costs.clone(),
        registers: 3,
        options: crate::model::passes::Options { max_unroll_iterations: 16, ..crate::model::passes::Options::default() },
        ..Where::default()
    };

    assert!(profit::spill_risk(&_pressured(), &costs, r#where.registers, None).unwrap() > 0);
    assert_eq!(_rejection(&_loop(), &_pressured(), 1, 2, &r#where), Some("growth"));
}

#[test]
fn test_spill_prone_complete_peel_respects_the_sequence_budget() {
    let costs = costs(|one| {
        one.add = 1;
        one.branch = 100;
        one.r#move = 1;
        one.load = 1;
        one.store = 1;
    });
    let r#where = Where {
        costs: costs.clone(),
        registers: 3,
        options: crate::model::passes::Options { max_unroll_iterations: 16, max_unrolled_operations: 200, ..crate::model::passes::Options::default() },
        ..Where::default()
    };

    assert!(profit::spill_risk(&_large_pressured(), &costs, r#where.registers, None).unwrap() > 0);
    assert_eq!(_rejection(&_loop(), &_large_pressured(), 1, 8, &r#where), Some("operation-growth"));
}

#[test]
fn test_structural_saving_must_pay_for_unavoidable_pressure() {
    let costs = costs(|one| {
        one.add = 1;
        one.branch = 2;
        one.r#move = 1;
        one.load = 10;
        one.store = 10;
    });
    let r#where = Where {
        costs,
        registers: 2,
        ..Where::default()
    };

    assert_eq!(_rejection(&_loop(), &_pressured(), 1, 8, &r#where), Some("pressure"));
}

#[test]
fn test_pressure_prices_independent_spill_waves() {
    let costs = costs(|one| {
        one.load = 10;
        one.store = 10;
    });

    assert_eq!(profit::spill_risk(&_pressure_waves(), &costs, 2, None), Some(40));
}

#[test]
fn test_integer_pressure_does_not_consume_x87_values() {
    let costs = costs(|one| {
        one.load = 10;
        one.store = 10;
    });

    assert_eq!(profit::spill_risk(&_floating_pressure(), &costs, 1, None), Some(0));
}
