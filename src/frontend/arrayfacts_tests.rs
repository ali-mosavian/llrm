//! Port of `tests/test_array_facts.py`: unknown branches must not erase
//! independently proven constant array extents.
//!
//! Skipped, needing `mir.bodies`, corpus loaders and `wholeseg`:
//! `test_guarded_record_stores_only_exclude_proven_disjoint_statics`,
//! `test_arrphi_proves_both_unknown_branches_and_the_join`,
//! `test_hugerg_has_an_inductive_extent_proof_with_a_small_budget`,
//! `test_inductive_proof_must_preserve_its_own_preconditions`.

use super::*;
use crate::frontend::addressfacts::region;
use crate::model::ir::Operation;
use crate::model::mir::{ArrayRequest, Const, MirBody, OpCode};

fn op(at: i64, operation: Operation, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
    let mut made = Op::new(at, OpCode::Operation(operation), "", defines, uses);
    made.kind = kind;
    made
}

fn diamond() -> RaisedBody {
    let descriptor = Symbol::new(Space::Segment, 5, 16, 2);
    let header = MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 16) }), 4);
    let request = ArrayRequest::new(descriptor, 2, vec![(1, 4)]);
    let mut allocation = op(0, Operation::Call, vec![], vec![], Kind::Call);
    allocation.array = Some(request);
    let field = MemRef { addr: Some(header.addr.unwrap().plus(14)), width: 2, ..header.clone() };
    allocation.memory_values = vec![(field, Const::new(4, 2))];
    let (pointer, offset) = (Value::new(1, 1), Value::new(2, 2));
    let mut base = op(1, Operation::Move, vec![pointer], vec![], Kind::Load);
    base.args = vec![Arg::Cell(Cell { r#ref: header.clone() })];
    base.results = vec![Arg::Held(Held { value: pointer, width: 4 })];
    base.loads = vec![header];
    let mut advance = op(2, Operation::Binary, vec![offset], vec![pointer], Kind::PtrOffset);
    advance.args = vec![Arg::Held(Held { value: pointer, width: 4 }), Arg::Const(Const::new(2, 4))];
    advance.results = vec![Arg::Held(Held { value: offset, width: 4 })];
    let mut cell = MemRef::new(None, 2);
    cell.base = Some(offset);
    cell.pointer = true;
    let store = |at: i64| {
        let mut made = op(at, Operation::Move, vec![], vec![offset], Kind::Store);
        made.args = vec![Arg::Const(Const::new(at, 2))];
        made.results = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
        made.stores = vec![cell.clone()];
        made
    };
    let value = Value::new(3, 30);
    let mut read = op(30, Operation::Move, vec![value], vec![offset], Kind::Load);
    read.args = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
    read.results = vec![Arg::Held(Held { value, width: 2 })];
    read.loads = vec![cell.clone()];
    RaisedBody::new(MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![allocation, base, advance], vec![10, 20]),
            MirBlock::new(10, vec![], vec![store(10)], vec![30]),
            MirBlock::new(20, vec![], vec![store(20)], vec![30]),
            MirBlock::new(30, vec![], vec![read], vec![]),
        ],
    ))
}

fn with_blocks(body: &RaisedBody, blocks: Vec<MirBlock>) -> RaisedBody {
    body.with_blocks(blocks)
}

#[test]
fn test_near_region_requires_nonwrapping_complete_access() {
    for (low, high, width) in [(-7, 0, 4), (0, 65530, 4), (0, 2, 0)] {
        let anchor = Addr { index: 5, ..Addr::new(Space::Segment, 6) };
        assert!(region(anchor, interval(low, high, 2), width).is_none(), "{low} {high} {width}");
    }
}

#[test]
fn test_unknown_branch_preserves_a_bounded_pointer() {
    for reverse in [false, true] {
        let mut body = diamond();
        if reverse {
            body = with_blocks(&body, body.blocks.iter().rev().cloned().collect());
        }
        let after = proven(body, 10000);
        let pointers: Vec<&MemRef> = after
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .flat_map(|op| op.loads.iter().chain(&op.stores))
            .filter(|reference| reference.pointer)
            .collect();
        assert_eq!(pointers.len(), 3, "{reverse}");
        assert!(pointers.iter().all(|reference| reference.allocation.is_some()), "{reverse}");
    }
}

#[test]
fn test_outside_the_extent_is_not_owned() {
    for offset in [-2_i64, 7, 8, 0x8000_0000] {
        let body = diamond();
        let block = &body.blocks[0];
        let mut ops = block.ops.clone();
        let last = ops.last_mut().unwrap();
        last.args = vec![last.args[0].clone(), Arg::Const(Const::new(offset, 4))];
        let mut blocks = body.blocks.clone();
        blocks[0] = block.with_ops(ops);
        let body = with_blocks(&body, blocks);
        assert_eq!(proven(body.clone(), 10000), body, "{offset}");
    }
}

#[test]
fn test_one_path_invalidating_allocation_prevents_join_proof() {
    let body = diamond();
    let right = &body.blocks[2];
    let call = op(21, Operation::Call, vec![], vec![], Kind::Call);
    let mut blocks = body.blocks.clone();
    blocks[2] = right.with_ops(right.ops.iter().cloned().chain([call]).collect());
    let after = proven(with_blocks(&body, blocks), 10000);
    assert!(after.blocks.last().unwrap().ops[0].loads[0].allocation.is_none());
}

#[test]
fn test_budget_exhaustion_adds_no_facts() {
    let body = diamond();
    assert_eq!(proven(body.clone(), 1), body);
}

#[test]
fn test_interval_cannot_acquire_unwritten_high_bits() {
    assert!(_fitted(Some(&Fact::Interval(interval(-1, 0, 2))), 4).is_none());
}

#[test]
fn test_copy_does_not_implicitly_extend_an_offset() {
    let (source, target) = (Value::new(50, 0), Value::new(51, 1));
    let mut copy = op(1, Operation::Move, vec![target], vec![source], Kind::Copy);
    copy.args = vec![Arg::Held(Held { value: source, width: 2 })];
    copy.results = vec![Arg::Held(Held { value: target, width: 4 })];
    assert!(_result(&copy, &[Some(Fact::Int(65535.into()))]).is_none());
}

/// A low word of 2 does not bound an offset whose high word is unknown.
#[test]
fn test_narrow_constant_does_not_prove_a_wider_offset() {
    let body = diamond();
    let entry = &body.blocks[0];
    let delta = Value::new(9, 2);
    let mut constant = op(2, Operation::Move, vec![delta], vec![], Kind::Copy);
    constant.args = vec![Arg::Const(Const::new(2, 2))];
    constant.results = vec![Arg::Held(Held { value: delta, width: 2 })];
    let mut ops = entry.ops.clone();
    let mut advance = ops.pop().unwrap();
    advance.args = vec![advance.args[0].clone(), Arg::Held(Held { value: delta, width: 4 })];
    ops.extend([constant, advance]);
    let mut blocks = body.blocks.clone();
    blocks[0] = entry.with_ops(ops);
    let body = with_blocks(&body, blocks);
    assert_eq!(proven(body.clone(), 10000), body);
}

/// Reusing a descriptor cannot make a pointer into its previous allocation valid.
#[test]
fn test_new_allocation_does_not_revive_a_stale_pointer() {
    let body = diamond();
    let entry = &body.blocks[0];
    let second = Op { at: 3, ..entry.ops[0].clone() };
    let mut blocks = body.blocks.clone();
    blocks[0] = entry.with_ops(entry.ops.iter().cloned().chain([second]).collect());
    let body = with_blocks(&body, blocks);
    assert_eq!(proven(body.clone(), 10000), body);
}
