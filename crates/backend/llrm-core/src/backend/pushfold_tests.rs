use std::sync::Arc;

use llrm_lir::registers::RegId;
use llrm_lir::{Held, Mem};

use super::folded;
use crate::model::ir::{Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

fn held(value: u32) -> Held {
    Held { value, width: 4 }
}

fn cell(
    base: u32,
    offset: i64,
) -> Mem {
    Mem { base: Some(held(base)), offset, ..Mem::new(None, 4) }
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
    Arc::new(Insn::new(
        at,
        Some((at, at)),
        Some(Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }),
        defines,
        uses,
    ))
}

/// `value := mov [base + offset]`.
fn load(
    at: i64,
    value: u32,
    base: u32,
    offset: i64,
) -> Arc<Insn> {
    insn(at, Operation::Move, "mov", vec![Loc::Held(held(value))], vec![Loc::Mem(cell(base, offset))], vec![value], vec![base])
}

fn push(
    at: i64,
    value: u32,
) -> Arc<Insn> {
    insn(at, Operation::Push, "push", vec![], vec![Loc::Held(held(value))], vec![], vec![value])
}

fn body(insns: Vec<Arc<Insn>>) -> LirBody {
    LirBody::new("g", 1, vec![LirBlock::new(1, insns).into()], IndexMap::default(), IndexMap::default())
}

fn names(body: &LirBody) -> Vec<String> {
    body.blocks[0].insns.iter().map(|one| one.what.as_ref().unwrap().name.as_deref().map(str::to_owned).unwrap()).collect()
}

/// `f(p->a, p->b, p->c)` pushed 3 loads into registers and 3 pushes of them
/// where the pin's `canFoldIntoRegPush` makes 3 `push [m]`.
#[test]
fn test_a_pushed_load_read_once_is_pushed_from_memory() {
    let before = body(vec![load(1, 2, 1, 8), push(2, 2), load(3, 3, 1, 4), push(4, 3), load(5, 4, 1, 0), push(6, 4)]);
    let after = folded(&before);
    assert_eq!(names(&after), ["push", "push", "push"]);
    let first = after.blocks[0].insns[0].what.as_ref().unwrap();
    assert_eq!(first.sources, vec![Loc::Mem(cell(1, 8))]);
    assert_eq!(after.blocks[0].insns[0].uses, vec![1]);
}

#[test]
fn test_a_value_read_twice_keeps_its_load() {
    let two = insn(5, Operation::Move, "mov", vec![Loc::Held(held(9))], vec![Loc::Held(held(2))], vec![9], vec![2]);
    let before = body(vec![load(1, 2, 1, 8), push(2, 2), two]);
    assert_eq!(names(&folded(&before)), ["mov", "push", "mov"]);
}

#[test]
fn test_a_store_between_the_load_and_the_push_keeps_the_load() {
    let store = insn(2, Operation::Move, "mov", vec![Loc::Mem(cell(7, 0))], vec![Loc::Held(held(8))], vec![], vec![7, 8]);
    let before = body(vec![load(1, 2, 1, 8), store, push(3, 2)]);
    assert_eq!(names(&folded(&before)), ["mov", "mov", "push"]);
}

#[test]
fn test_a_call_between_the_load_and_the_push_keeps_the_load() {
    let call = insn(2, Operation::Call, "call", vec![], vec![], vec![], vec![]);
    let before = body(vec![load(1, 2, 1, 8), call, push(3, 2)]);
    assert_eq!(names(&folded(&before)), ["mov", "call", "push"]);
}

#[test]
fn test_a_volatile_load_keeps_its_register() {
    let mut volatile = (*load(1, 2, 1, 8)).clone();
    volatile.volatile = true;
    let before = body(vec![Arc::new(volatile), push(2, 2)]);
    assert_eq!(names(&folded(&before)), ["mov", "push"]);
}

/// The pushes of the sequence move the stack pointer: a cell addressed through
/// it is not the same cell at the push.
#[test]
fn test_a_cell_addressed_through_the_stack_pointer_keeps_its_load() {
    let stack = Mem { through: RegId::SP, ..Mem::new(None, 4) };
    let from_stack = insn(1, Operation::Move, "mov", vec![Loc::Held(held(2))], vec![Loc::Mem(stack)], vec![2], vec![]);
    let before = body(vec![from_stack, push(2, 2)]);
    assert_eq!(names(&folded(&before)), ["mov", "push"]);
}
