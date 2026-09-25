//! Port of `tests/test_canonical.py`.

use std::collections::BTreeMap;
use std::rc::Rc;

use num_bigint::BigInt;

use super::identities;
use crate::model::execute::{Memory, run};
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Const, Held, Kind, MirBlock, MirBody, Op, OpCode, Value};
use crate::support::hash::IndexMap;

/// `t = x kind constant; if t test 0 return 1 else return 2`, x live in.
fn _body(kind: Kind, constant: i64, test: Kind) -> MirBody {
    let (x, t) = (Value::new(1, 0), Value::new(2, 0));
    let flags = Value { flags: true, ..Value::new(3, 0) };
    let mut compare = Op::new(0, OpCode::Operation(Operation::Compare), "", vec![flags], vec![t]);
    compare.kind = Kind::Sub;
    compare.args = vec![Arg::Held(Held { value: t, width: 2 }), Arg::Const(Const::new(0, 2))];
    let mut branch = Op::new(0, OpCode::Operation(Operation::Branch), "", vec![], vec![flags]);
    branch.kind = Kind::Branch;
    branch.test = Some(test);
    branch.target = Some(1);
    let ops = vec![
        mir::computed(0, kind, t, vec![Arg::Held(Held { value: x, width: 2 }), Arg::Const(Const::new(constant, 2))], 2),
        compare,
        branch,
    ];
    let returned = |at: i64| {
        let mut op = Op::new(at, OpCode::Operation(Operation::Return), "", vec![], vec![]);
        op.kind = Kind::Return;
        op.args = vec![Arg::Const(Const::new(at, 2))];
        op
    };
    MirBody {
        sealed: true,
        ..MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], ops, vec![1, 2]),
                MirBlock::new(1, vec![], vec![returned(1)], vec![]),
                MirBlock::new(2, vec![], vec![returned(2)], vec![]),
            ],
        )
    }
}

fn returned(body: &MirBody, x: i64) -> Vec<BigInt> {
    let inputs = IndexMap::from_iter([(Value::new(1, 0), BigInt::from(x))]);
    run(body, &inputs, &Memory::default(), None, 1_000_000, &BTreeMap::new()).expect("runs").returned
}

/// Rotation's `bound - 0 + 0` and `bound <=u 0` used to be folded inside the rewrite that wrote them.
#[test]
fn test_a_neutral_term_is_its_operand_and_a_test_below_zero_is_equality() {
    for (kind, constant) in [(Kind::Add, 0), (Kind::Sub, 0), (Kind::Mul, 1)] {
        for (test, equality) in [(Kind::BelowEq, Kind::Eq), (Kind::Above, Kind::Ne)] {
            let body = Rc::new(_body(kind, constant, test));
            let folded = identities(body.clone());
            let entry = &folded.blocks[0];
            assert_eq!(entry.ops.iter().map(|op| op.kind).collect::<Vec<_>>(), [Kind::Sub, Kind::Branch]);
            assert_eq!(entry.ops[1].test, Some(equality));
            for x in [0, 1, 0xFFFF] {
                assert_eq!(returned(&folded, x), returned(&body, x), "{kind:?} {test:?} {x}");
            }
        }
    }
}

#[test]
fn test_a_term_that_is_not_neutral_stays() {
    let body = Rc::new(_body(Kind::Sub, 1, Kind::Below));
    assert!(Rc::ptr_eq(&identities(body.clone()), &body));
}

/// NESTED's raised `add x,0` vanished outright and its bytes were refused as not instructions.
#[test]
fn test_a_folded_term_keeps_the_source_bytes_it_owned() {
    let mut body = _body(Kind::Add, 0, Kind::Below);
    body.blocks[0].ops[0].id = Some(7);
    body.blocks[0].ops[0].source_backed = true;

    let folded = identities(Rc::new(body));
    let first = &folded.blocks[0].ops[0];
    assert_eq!((first.kind, first.id), (Kind::Nothing, Some(7)));
}
