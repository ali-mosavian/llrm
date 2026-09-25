//! Port of `tests/test_float_cse_paths.py`'s numeric-policy test.
//!
//! The file's other tests exercise `transform.subexpressions` and
//! `floatbounds.exact` alone and belong to those modules' ports.

use std::collections::BTreeSet;
use std::rc::Rc;

use super::*;
use crate::model::floating::{Format, Precision, Rounding, Semantics};
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Cell, Const, Held, MemRef, MirBlock, MirBody, Op, OpCode, Value};
use crate::objectfile::module::{Addr, Space};
use crate::optimize::transform;

fn body_with_path() -> MirBody {
    let [source, first, second] = [1u32, 2, 3].map(|n| Value { variable: n, ..Value::new(n, i64::from(n)) });
    let mut load = Op::new(0, OpCode::Operation(Operation::FloatLoad), "fild", vec![source], Vec::new());
    load.kind = Kind::Fload;
    load.args = vec![Arg::Const(Const::new(7, 2))];
    load.results = vec![Arg::Held(Held { value: source, width: 10 })];
    load.floating = Some(Semantics::new([Format::Signed16], Format::Extended80, Precision::Exact, Rounding::None));

    let add = |at: i64, result: Value| {
        let mut op = Op::new(at, OpCode::Operation(Operation::FloatArith), "fadd", vec![result], vec![source]);
        op.kind = Kind::Fadd;
        op.args = vec![Arg::Held(Held { value: source, width: 10 }), Arg::Held(Held { value: source, width: 10 })];
        op.results = vec![Arg::Held(Held { value: result, width: 10 })];
        op.floating = Some(Semantics::new(
            [Format::Extended80, Format::Extended80],
            Format::Extended80,
            Precision::Dynamic,
            Rounding::Dynamic,
        ));
        op
    };

    let mut used = Op::new(8, OpCode::Operation(Operation::Nothing), "", Vec::new(), vec![second]);
    used.kind = Kind::Arg;
    used.args = vec![Arg::Held(Held { value: second, width: 10 })];
    MirBody::new(
        0,
        vec![
            MirBlock::new(0, Vec::new(), vec![load, add(2, first)], vec![4, 5]),
            MirBlock::new(4, Vec::new(), Vec::new(), vec![6]),
            MirBlock::new(5, Vec::new(), Vec::new(), vec![6]),
            MirBlock::new(6, Vec::new(), vec![add(6, second), used], Vec::new()),
        ],
    )
}

#[test]
fn test_deferred_runtime_float_reuse_respects_environment() {
    for interruption in [None, Some(Kind::Call), Some(Kind::Opaque), Some(Kind::Fcheck)] {
        let mut body = body_with_path();
        let cell = MemRef::new(Some(Addr { index: 1, ..Addr::new(Space::Segment, 0) }), 4);
        let load = &mut body.blocks[0].ops[0];
        load.args = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
        load.loads = vec![cell];
        load.floating = Some(Semantics::new([Format::Binary32], Format::Extended80, Precision::Exact, Rounding::None));
        let mut body = checkpoints(RaisedBody::new(body)).body;
        if let Some(kind) = interruption {
            let mut middle = Op::new(4, OpCode::Operation(Operation::Nothing), "", Vec::new(), Vec::new());
            middle.kind = kind;
            body.blocks[1].ops = vec![middle];
        }
        let after = transform::subexpressions(&Rc::new(body), &BTreeSet::new(), false).unwrap();
        let count = after.blocks.iter().flat_map(|block| &block.ops).filter(|op| op.kind == Kind::Fadd).count();
        assert_eq!(count, if interruption.is_none() { 1 } else { 2 }, "{interruption:?}");
    }
}
