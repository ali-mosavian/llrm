//! Port of `tests/test_lcssa.py`. `is` assertions compare by `==`.

use std::rc::Rc;
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Const, Held, Kind, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value};

use super::closed;

pub fn value(id: u32, at: i64, variable: u32, version: u32) -> Value {
    Value { id, at, flags: false, variable, version }
}

pub fn incoming(pairs: &[(i64, Value)]) -> OrderedMap<i64, Value> {
    pairs.iter().copied().collect()
}

pub fn operation(at: i64, kind: Kind, defines: &[Value], uses: &[Value], args: Vec<Arg>, results: Vec<Arg>) -> Op {
    let mut op = Op::new(at, OpCode::Operation(Operation::Move), "mov", defines.to_vec(), uses.to_vec());
    op.kind = kind;
    op.args = args;
    op.results = results;
    op
}

pub fn held(value: Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

pub fn loop_with_exit_use() -> (MirBody, Value, Op) {
    let seed = value(1, 0, 1, 1);
    let carried = value(2, 1, 1, 2);
    let stepped = value(3, 2, 1, 3);
    let answer = value(4, 3, 2, 1);
    let initialize = operation(0, Kind::Copy, &[seed], &[], vec![Arg::Const(Const::new(0, 2))], vec![held(seed, 2)]);
    let advance = operation(
        2,
        Kind::Add,
        &[stepped],
        &[carried],
        vec![held(carried, 2), Arg::Const(Const::new(1, 2))],
        vec![held(stepped, 2)],
    );
    let consume = operation(3, Kind::Copy, &[answer], &[carried], vec![held(carried, 2)], vec![held(answer, 2)]);
    let body = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![initialize], vec![1]),
            MirBlock::new(1, vec![Phi { result: carried, incoming: incoming(&[(0, seed), (2, stepped)]) }], vec![], vec![2, 3]),
            MirBlock::new(2, vec![], vec![advance], vec![1]),
            MirBlock::new(3, vec![], vec![consume.clone()], vec![]),
        ],
    );
    (body, carried, consume)
}

#[test]
fn test_a_loop_value_used_after_the_exit_gets_an_exit_phi() {
    let (body, carried, consume) = loop_with_exit_use();

    let result = closed(&Rc::new(MirBody::clone(&body))).unwrap();

    let exit_block = result.block(3).unwrap();
    assert_eq!(exit_block.phis.len(), 1);
    let phi = &exit_block.phis[0];
    assert_eq!(phi.incoming, incoming(&[(1, carried)]));
    assert_eq!(phi.result.variable, carried.variable);
    assert!(phi.result.version > carried.version);
    let changed = &exit_block.ops[0];
    assert_ne!(*changed, consume);
    assert_eq!(changed.uses, vec![phi.result]);
    assert_eq!(changed.args, vec![held(phi.result, 2)]);
}

#[test]
fn test_loop_closed_ssa_is_idempotent() {
    let (body, _, _) = loop_with_exit_use();
    let once = closed(&Rc::new(MirBody::clone(&body))).unwrap();
    assert_eq!(closed(&once).unwrap(), once);
}

#[test]
fn test_exit_edge_into_a_bypass_join_is_closed_once() {
    let (mut body, carried, _) = loop_with_exit_use();
    let seed = body.blocks[0].ops[0].defines[0];
    let answer = value(9, 4, 1, 4);
    body.blocks[0].succ = vec![1, 4];
    body.blocks[3] = MirBlock::new(3, vec![], vec![], vec![4]);
    body.blocks.push(MirBlock::new(
        4,
        vec![Phi { result: answer, incoming: incoming(&[(0, seed), (3, carried)]) }],
        vec![],
        vec![],
    ));
    let result = closed(&Rc::new(MirBody::clone(&body))).unwrap();
    let exit_value = result.block(3).unwrap().phis[0].result;
    assert_eq!(result.block(4).unwrap().phis[0].incoming, incoming(&[(0, seed), (3, exit_value)]));
    assert_eq!(closed(&result).unwrap(), result);
}

#[test]
fn test_a_value_already_consumed_by_an_exit_phi_is_closed() {
    let (body, carried, _) = loop_with_exit_use();
    let result = value(8, 3, 8, 1);
    let exit_at = body.block(3).unwrap().at;
    let body = MirBody::new(
        body.entry,
        body.blocks
            .iter()
            .map(|block| {
                if block.at == exit_at {
                    MirBlock::new(
                        block.at,
                        vec![Phi { result, incoming: incoming(&[(1, carried)]) }],
                        vec![],
                        block.succ.clone(),
                    )
                } else {
                    block.clone()
                }
            })
            .collect(),
    );
    assert_eq!(closed(&Rc::new(MirBody::clone(&body))).unwrap(), Rc::new(body));
}

#[test]
fn test_multiple_edges_to_one_dedicated_exit_are_closed() {
    let (body, carried, _) = loop_with_exit_use();
    let latch_at = body.block(2).unwrap().at;
    let body = MirBody::new(
        body.entry,
        body.blocks
            .iter()
            .map(|block| {
                if block.at == latch_at {
                    MirBlock::new(block.at, block.phis.clone(), block.ops.clone(), vec![1, 3])
                } else {
                    block.clone()
                }
            })
            .collect(),
    );
    let result = closed(&Rc::new(MirBody::clone(&body))).unwrap();
    let exit_block = result.block(3).unwrap();
    assert_eq!(exit_block.phis.len(), 1);
    assert_eq!(exit_block.phis[0].incoming, incoming(&[(1, carried), (2, carried)]));
    assert_eq!(exit_block.ops[0].uses, vec![exit_block.phis[0].result]);
    assert_eq!(closed(&result).unwrap(), result);
}

#[test]
fn test_exit_phi_cannot_read_a_value_missing_on_one_edge() {
    let (mut body, _, consume) = loop_with_exit_use();
    let stepped = body.blocks[2].ops[0].defines[0];
    body.blocks[2].succ = vec![1, 3];
    let mut changed = consume;
    changed.uses = vec![stepped];
    changed.args = vec![held(stepped, 2)];
    body.blocks[3].ops = vec![changed];
    assert_eq!(closed(&Rc::new(MirBody::clone(&body))).unwrap(), Rc::new(body));
}

#[test]
fn test_exit_shared_with_a_bypass_still_requires_canonicalization() {
    let (mut body, _, _) = loop_with_exit_use();
    body.blocks[0].succ = vec![1, 3];
    assert_eq!(closed(&Rc::new(MirBody::clone(&body))).unwrap(), Rc::new(body));
}

/// matmul8: peeling left the outer counter's step behind `phi(x, x)`, and
/// after unrolling an inner loop its exit phi stayed: no trip count through
/// the one, no unrolling past the other, so both loop nests stayed rolled.
#[test]
fn test_a_phi_naming_one_value_outside_a_loop_exit_is_that_value() {
    let x = value(1, 0, 1, 1);
    let joined = value(2, 3, 1, 2);
    let copied = value(3, 4, 1, 3);
    let read = value(4, 4, 2, 1);
    let make = operation(0, Kind::Copy, &[x], &[], vec![Arg::Const(Const::new(7, 2))], vec![held(x, 2)]);
    let consume = operation(4, Kind::Copy, &[read], &[copied], vec![held(copied, 2)], vec![held(read, 2)]);
    let body = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![make], vec![1, 2]),
            MirBlock::new(1, vec![], vec![], vec![3]),
            MirBlock::new(2, vec![], vec![], vec![3]),
            MirBlock::new(3, vec![Phi { result: joined, incoming: incoming(&[(1, x), (2, x)]) }], vec![], vec![4]),
            MirBlock::new(4, vec![Phi { result: copied, incoming: incoming(&[(3, joined)]) }], vec![consume], vec![]),
        ],
    );

    let result = closed(&Rc::new(body)).unwrap();

    assert!(result.blocks.iter().all(|block| block.phis.is_empty()), "{result:?}");
    assert_eq!(result.block(4).unwrap().ops[0].uses, vec![x]);
}
