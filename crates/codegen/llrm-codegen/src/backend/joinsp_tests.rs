//! Tests of `joinsp`, the join of a copy of the stack pointer with it.
use std::sync::Arc;

use llrm_lir::Held;
use llrm_lir::registers::RegId;

use super::joined;
use crate::model::ir::{Loc, Operation, Reg, Semantics};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

const SP: RegId = RegId::RSP;

fn held(value: u32) -> Loc {
    Loc::Held(Held { value, width: 8, offset: 0 })
}

fn sp() -> Loc {
    Loc::Reg(Reg { register: SP, width: 8 })
}

fn insn(
    at: i64,
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
    defines: Vec<u32>,
    uses: Vec<u32>,
) -> Arc<Insn> {
    let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
}

fn save(at: i64) -> Arc<Insn> {
    insn(at, Operation::Move, "mov", vec![held(1)], vec![sp()], vec![1], vec![])
}

fn restore(at: i64) -> Arc<Insn> {
    insn(at, Operation::Move, "mov", vec![sp()], vec![held(1)], vec![], vec![1])
}

fn work(at: i64) -> Arc<Insn> {
    insn(at, Operation::Binary, "add", vec![held(2)], vec![held(2), held(3)], vec![2], vec![2, 3])
}

fn call(at: i64) -> Arc<Insn> {
    insn(at, Operation::Call, "call", vec![], vec![], vec![], vec![])
}

/// save, a loop block, restore, call.
fn body(between: Vec<Arc<Insn>>) -> LirBody {
    let first = LirBlock { succ: vec![1].into(), ..LirBlock::new(0, vec![save(0)]) };
    let mut looped = between;
    looped.push(insn(90, Operation::Branch, "jne", vec![], vec![], vec![], vec![]));
    let second = LirBlock { succ: vec![1, 2].into(), ..LirBlock::new(1, looped) };
    let third = LirBlock::new(2, vec![restore(100), call(101)]);
    LirBody::new(
        "t",
        0,
        vec![Block::from(first), Block::from(second), Block::from(third)],
        IndexMap::default(),
        IndexMap::default(),
    )
}

fn count(
    body: &LirBody,
    name: &str,
) -> usize {
    body.blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some(name)))
        .count()
}

/// vla_callee_constant x86_64 -O2: the save and restore of a static frame
/// stayed, and the write to `rsp` kept a frame register.
#[test]
fn a_save_and_restore_with_no_stack_write_between_go() {
    let out = joined(&body(vec![work(10)]), SP);
    assert_eq!(count(&out, "mov"), 0);
    assert_eq!(count(&out, "call"), 1);
}

#[test]
fn a_call_between_keeps_both() {
    let out = joined(&body(vec![work(10), call(11)]), SP);
    assert_eq!(count(&out, "mov"), 2);
}

#[test]
fn a_push_between_keeps_both() {
    let push = insn(11, Operation::Push, "push", vec![], vec![held(3)], vec![], vec![3]);
    let out = joined(&body(vec![push]), SP);
    assert_eq!(count(&out, "mov"), 2);
}

#[test]
fn a_write_to_the_stack_pointer_between_keeps_both() {
    let sub = insn(11, Operation::Binary, "sub", vec![sp()], vec![sp(), held(3)], vec![], vec![3]);
    let out = joined(&body(vec![sub]), SP);
    assert_eq!(count(&out, "mov"), 2);
}

#[test]
fn a_saved_value_read_otherwise_stays() {
    let read = insn(11, Operation::Binary, "add", vec![held(4)], vec![held(1), held(3)], vec![4], vec![1, 3]);
    let out = joined(&body(vec![read]), SP);
    assert_eq!(count(&out, "mov"), 2);
}

/// The selector hands the save over through a copy of a copy (`%2 = COPY %1`,
/// `$rsp = COPY %2`): the first x86_64 -O2 run joined nothing because the
/// restore named another value than the save.
#[test]
fn a_restore_of_a_copy_of_the_save_goes_with_it() {
    let copy = insn(1, Operation::Move, "mov", vec![held(5)], vec![held(1)], vec![5], vec![1]);
    let restore5 = insn(100, Operation::Move, "mov", vec![sp()], vec![held(5)], vec![], vec![5]);
    let mut b = body(vec![work(10)]);
    let mut first: Vec<Arc<Insn>> = b.blocks[0].insns.iter().cloned().collect();
    first.push(copy);
    b.blocks[0] = b.blocks[0].with_insns(first);
    let mut last: Vec<Arc<Insn>> = b.blocks[2].insns.iter().cloned().collect();
    last[0] = restore5;
    b.blocks[2] = b.blocks[2].with_insns(last);
    let out = joined(&b, SP);
    assert_eq!(count(&out, "mov"), 0);
}
