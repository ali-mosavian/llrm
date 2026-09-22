//! The `profit`-only cases of `tests/test_unroll_budget.py` (there is no
//! `tests/test_profit.py`): test_pressure_prices_independent_spill_waves,
//! test_integer_pressure_does_not_consume_x87_values. The rest need `unroll`.

use super::spill_risk;
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Const, Held, Kind, MirBlock, MirBody, Op, OpCode, Value};
use crate::model::passes::OperationCosts;

fn _pressure_waves() -> MirBody {
    let mut ops = Vec::new();
    let mut fresh = 1;
    for wave in 0..2_i64 {
        let values = (0..3).map(|index| Value::new(fresh + index as u32, wave * 10 + index)).collect::<Vec<_>>();
        fresh += values.len() as u32;
        for (index, value) in values.iter().enumerate() {
            let mut op = Op::new(value.at, OpCode::Operation(Operation::Binary), "add", vec![*value], vec![]);
            op.kind = Kind::Add;
            op.args = vec![Arg::Const(Const::new(index as i64, 2)), Arg::Const(Const::new(wave, 2))];
            op.results = vec![Arg::Held(Held { value: *value, width: 2 })];
            ops.push(op);
        }
        let mut op = Op::new(wave * 10 + 4, OpCode::Operation(Operation::Binary), "add", vec![], values.clone());
        op.kind = Kind::Add;
        op.args = values.iter().map(|value| Arg::Held(Held { value: *value, width: 2 })).collect();
        ops.push(op);
    }
    MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])])
}

fn _floating_pressure() -> MirBody {
    let values = (1..4).map(|index| Value::new(index, i64::from(index))).collect::<Vec<_>>();
    let mut ops = values
        .iter()
        .map(|value| {
            let mut op = Op::new(value.at, OpCode::Operation(Operation::FloatLoad), "fld", vec![*value], vec![]);
            op.kind = Kind::Fload;
            op.results = vec![Arg::Held(Held { value: *value, width: 10 })];
            op
        })
        .collect::<Vec<_>>();
    let mut consume = Op::new(4, OpCode::Operation(Operation::Binary), "fadd", vec![], values.clone());
    consume.kind = Kind::Fadd;
    consume.args = values.iter().map(|value| Arg::Held(Held { value: *value, width: 10 })).collect();
    ops.push(consume);
    MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])])
}

#[test]
fn test_pressure_prices_independent_spill_waves() {
    let costs = OperationCosts { load: 10, store: 10, ..OperationCosts::default() };

    assert_eq!(spill_risk(&_pressure_waves(), &costs, 2, None), Some(40));
}

#[test]
fn test_integer_pressure_does_not_consume_x87_values() {
    let costs = OperationCosts { load: 10, store: 10, ..OperationCosts::default() };

    assert_eq!(spill_risk(&_floating_pressure(), &costs, 1, None), Some(0));
}
