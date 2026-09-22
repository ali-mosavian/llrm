//! Port of `tests/test_raising_words.py` and `tests/test_word_arithmetic_carry.py`.
//!
//! Skipped, needing `mir.bodies` and `corpus.loaded`:
//! `test_culling_pointer_arithmetic_preserves_the_returned_upper_word`,
//! `test_culling_restore_carries_the_original_upper_word`.

use iced_x86::Register;

use super::*;
use crate::model::ir::Operation;
use crate::model::mir::{Const, MirBlock, MirBody, OpCode, Phi};

fn body_with_reader(width: u32) -> (RaisedBody, Value, Value) {
    let (old, result, output) = (Value::new(1, 1), Value::new(2, 2), Value::new(3, 3));
    let mut copy = Op::new(2, OpCode::Operation(Operation::Move), "", vec![result], vec![old]);
    copy.kind = Kind::Copy;
    copy.args = vec![Arg::Const(Const::new(7, 2))];
    copy.results = vec![Arg::Held(Held { value: result, width: 2 })];
    copy.merges = [(old, result)].into_iter().collect();
    let mut read = Op::new(3, OpCode::Operation(Operation::Move), "", vec![output], vec![result]);
    read.kind = Kind::Copy;
    read.args = vec![Arg::Held(Held { value: result, width })];
    read.results = vec![Arg::Held(Held { value: output, width })];
    (RaisedBody::new(MirBody::new(0, vec![MirBlock::new(0, vec![], vec![copy, read], vec![])])), old, result)
}

fn merged(old: Value, result: Value) -> OrderedMap<Value, Value> {
    [(old, result)].into_iter().collect()
}

/// IVARM's INTEGER branch load kept its redundant counter alive through a dead upper half.
#[test]
fn test_unused_upper_word_does_not_keep_the_old_definition() {
    let (body, old, _) = body_with_reader(2);
    let first = scalar(body).blocks[0].ops[0].clone();
    assert!(first.merges.is_empty() && !first.uses.contains(&old));
}

#[test]
fn test_wide_reader_keeps_the_preserved_upper_word() {
    let (body, old, result) = body_with_reader(4);
    assert_eq!(scalar(body).blocks[0].ops[0].merges, merged(old, result));
}

#[test]
fn test_unknown_reader_keeps_the_preserved_upper_word() {
    let (body, old, result) = body_with_reader(2);
    let (first, mut read) = (body.blocks[0].ops[0].clone(), body.blocks[0].ops[1].clone());
    read.kind = Kind::Opaque;
    read.args = vec![];
    read.results = vec![];
    let block = body.blocks[0].with_ops(vec![first, read]);
    let body = body.with_blocks(vec![block]);
    assert_eq!(scalar(body).blocks[0].ops[0].merges, merged(old, result));
}

#[test]
fn test_body_exit_keeps_the_preserved_upper_word() {
    let (mut body, old, result) = body_with_reader(2);
    body.origin = [(old, Register::EAX), (result, Register::EAX)].into_iter().collect();
    assert_eq!(scalar(body).blocks[0].ops[0].merges, merged(old, result));
}

#[test]
fn test_wide_phi_reader_keeps_the_preserved_upper_word() {
    let (body, old, result) = body_with_reader(4);
    let (first, mut read) = (body.blocks[0].ops[0].clone(), body.blocks[0].ops[1].clone());
    let joined = Value::new(4, 4);
    read.args = vec![Arg::Held(Held { value: joined, width: 4 })];
    read.uses = vec![joined];
    let mut phi = Phi::new(joined);
    phi.incoming.insert(0, result);
    let body = body.with_blocks(vec![
        MirBlock::new(0, vec![], vec![first], vec![4]),
        MirBlock::new(4, vec![phi], vec![read], vec![]),
    ]);
    assert_eq!(scalar(body).blocks[0].ops[0].merges, merged(old, result));
}
