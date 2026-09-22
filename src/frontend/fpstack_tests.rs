//! Port of `tests/test_fpstack.py`.
//!
//! Skipped, the module's autouse fixture monkeypatching
//! `raising_float_values.raised` out of `mir.bodies`:
//! `test_fpcse_store_reads_product_not_original_load`.
//! Skipped, needing `tools/stages.py` and that fixture:
//! `test_stage_dump_exposes_floating_value_chain`.

use super::*;
use crate::model::mir::{Const, MirBlock, Op, Opaque};

fn op(at: i64, shape: Operation, kind: Kind, args: Vec<Arg>, results: Vec<Arg>, stack: i64) -> Op {
    let mut made = Op::new(at, OpCode::Operation(shape), kind.to_string(), vec![], vec![]);
    made.kind = kind;
    made.args = args;
    made.results = results;
    made.stack = Some(stack);
    made
}

fn st(index: u32) -> Arg {
    Arg::Opaque(Opaque::named(None, format!("st{index}")))
}

fn body(ops: Vec<Op>) -> MirBody {
    MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])])
}

/// An arithmetic pop must leave its new result, not the previous destination, at the top.
#[test]
fn test_arithmetic_pop_writes_destination_before_renumbering() {
    let zero = || Arg::Const(Const::new(0, 4));
    let ops = vec![
        op(0, Operation::FloatLoad, Kind::Fload, vec![zero()], vec![st(0)], 1),
        op(1, Operation::FloatLoad, Kind::Fload, vec![zero()], vec![st(0)], 1),
        op(2, Operation::FloatArithPop, Kind::Fadd, vec![st(1), st(0)], vec![st(1)], -1),
        op(3, Operation::FloatUnary, Kind::Fneg, vec![st(0)], vec![st(0)], 0),
    ];
    let readings = readings(&body(ops));
    let expected: IndexMap<i64, Float> =
        [(1, readings[&0].defines.unwrap()), (0, readings[&1].defines.unwrap())].into_iter().collect();
    assert_eq!(readings[&2].uses, expected);
    assert!(readings[&2].defines.is_some());
    assert_eq!(readings[&2].popped, vec![readings[&1].defines.unwrap()]);
    let top: IndexMap<i64, Float> = [(0, readings[&2].defines.unwrap())].into_iter().collect();
    assert_eq!(readings[&3].uses, top);
    assert!(readings[&3].defines.is_some() && readings[&3].defines != readings[&2].defines);
}

/// A call or a ninth outstanding push invalidates the floating value graph.
#[test]
fn test_unknown_stack_does_not_claim_value_reuse() {
    for boundary in ["call", "overflow"] {
        let push = |at| op(at, Operation::FloatLoad, Kind::Fload, vec![Arg::Const(Const::new(0, 4))], vec![st(0)], 1);
        let mut ops = vec![push(0)];
        if boundary == "call" {
            let mut call = Op::new(1, OpCode::Operation(Operation::Call), "call", vec![], vec![]);
            call.kind = Kind::Call;
            ops.push(call);
        } else {
            ops.extend((1..9).map(push));
        }
        ops.push(op(10, Operation::FloatUnary, Kind::Fneg, vec![st(0)], vec![st(0)], 0));
        let readings = readings(&body(ops));
        assert!(readings[&0].defines.is_some(), "{boundary}");
        assert!(readings[&10].defines.is_none() && readings[&10].uses.is_empty(), "{boundary}");
    }
}
