use std::rc::Rc;

use crate::model::ir::Operation;
use crate::model::mir::{cleared, identified, transformed, Kind, Ledger, MirBlock, MirBody, Op, OpCode};

/// An operation owning the source bytes at `at`.
fn op(at: i64, name: &str) -> Op {
    Op { absorbed: vec![at as u32], ..Op::new(at, OpCode::Operation(Operation::Move), name, vec![], vec![]) }
}

fn block(at: i64, ops: Vec<Op>, succ: &[i64]) -> MirBlock {
    MirBlock::new(at, vec![], ops, succ.to_vec())
}

/// Each block's address and where its operations' bytes came from.
fn layout(body: &MirBody) -> Vec<(i64, Vec<i64>)> {
    body.blocks.iter().map(|block| (block.at, block.ops.iter().map(|op| op.at).collect())).collect()
}

/// `raw` entering the pipeline, then `pass` over it: the body the backend gets.
fn materialized(raw: MirBody, pass: impl Fn(&MirBody) -> MirBody) -> MirBody {
    let raw = identified(Rc::new(raw));
    let (entered, entry) = transformed(&raw, Rc::clone(&raw));
    let (done, stage) = transformed(&entered, Rc::new(pass(&entered)));
    Ledger::from_stages([&entry, &stage]).materialized(&done).unwrap()
}

/// `body` with block `from` appended to the entry block and gone.
fn merged(body: &MirBody, from: i64) -> MirBody {
    let moved = body.block(from).unwrap().ops.clone();
    let blocks = body
        .blocks
        .iter()
        .filter(|block| block.at != from)
        .map(|block| if block.at == body.entry { block.with_ops([block.ops.clone(), moved.clone()].concat()) } else { block.clone() })
        .collect();
    body.with_blocks(blocks)
}

/// Taken for a tombstone, a source `nop` left its block empty and
/// unreachable, and 8 builds were refused.
#[test]
fn test_a_source_nop_is_an_instruction_not_a_tombstone() {
    let nop = Op { kind: Kind::Nothing, ..op(0x31, "nop") };
    let (body, stage) = transformed(&MirBody::new(0x30, vec![]), identified(Rc::new(MirBody::new(0x30, vec![block(0x30, vec![op(0x30, "mov"), nop], &[])]))));
    assert!(stage.retired.is_empty(), "{:?}", stage.retired);
    assert_eq!(layout(&body), [(0x30, vec![0x30, 0x31])]);
}

/// A merged block's operations carry its bytes. Sent to where the block had
/// been instead, line numbers of merged code collapsed onto one address.
#[test]
fn test_bytes_move_with_the_block_they_were_merged_into() {
    let raw = MirBody::new(0x30, vec![block(0x30, vec![op(0x30, "mov")], &[0x40]), block(0x40, vec![cleared(&op(0x40, "mov")), op(0x42, "dropped"), op(0x44, "mov")], &[])]);
    let done = materialized(raw, |body| {
        let body = merged(body, 0x40);
        let ops = body.blocks[0].ops.iter().filter(|one| one.name != "dropped").cloned().collect();
        body.with_blocks(vec![body.blocks[0].with_ops(ops)])
    });
    assert_eq!(layout(&done), [(0x30, vec![0x30, 0x40, 0x44])]);
}

/// A vanished block's end is just after its last operation, wherever that
/// went. Kept as a block of its own, hotlop's lines 10 and 11 were placed
/// after line 14.
#[test]
fn test_bytes_at_a_vanished_blocks_end_follow_its_last_operation() {
    let raw = MirBody::new(
        0x30,
        vec![
            block(0x30, vec![op(0x30, "mov")], &[0x60]),
            block(0x48, vec![cleared(&op(0x48, "mov")), op(0x57, "mov"), cleared(&op(0x5a, "mov"))], &[]),
            block(0x60, vec![op(0x60, "mov")], &[]),
        ],
    );
    let done = materialized(raw, |body| merged(body, 0x48));
    assert_eq!(layout(&done), [(0x30, vec![0x30, 0x48, 0x57, 0x5a]), (0x60, vec![0x60])]);
}
