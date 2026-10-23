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
    insn(
        at,
        Operation::Move,
        "mov",
        vec![Loc::Held(held(value))],
        vec![Loc::Mem(cell(base, offset))],
        vec![value],
        vec![base],
    )
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
    body.blocks[0]
        .insns
        .iter()
        .map(|one| one.what.as_ref().unwrap().name.as_deref().map(str::to_owned).unwrap())
        .collect()
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
    let store =
        insn(2, Operation::Move, "mov", vec![Loc::Mem(cell(7, 0))], vec![Loc::Held(held(8))], vec![], vec![7, 8]);
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

/// `dest := mov src`, no load.
fn copy(
    at: i64,
    dest: u32,
    src: u32,
) -> Arc<Insn> {
    insn(at, Operation::Move, "mov", vec![Loc::Held(held(dest))], vec![Loc::Held(held(src))], vec![dest], vec![src])
}

fn add(
    at: i64,
    dest: u32,
    first: u32,
    second: u32,
    name: &str,
) -> Arc<Insn> {
    insn(
        at,
        Operation::Binary,
        name,
        vec![Loc::Held(held(dest))],
        vec![Loc::Held(held(first)), Loc::Held(held(second))],
        vec![dest],
        vec![first, second],
    )
}

/// `a + [m]` and `[m] + a`: `reassociate@696` (#3026) put the load first and
/// the post-allocation fold wanted it second, so `mov ecx, [esp+4]; add ecx,
/// eax` stayed where `add eax, [esp+4]` was listed: 8 instructions, was 6.
#[test]
fn test_a_load_is_the_memory_operand_of_an_add_whichever_operand_it_is() {
    let second = folded(&body(vec![copy(0, 9, 8), load(1, 2, 1, 8), add(2, 3, 9, 2, "add")]));
    assert_eq!(names(&second), ["mov", "add"]);
    let what = second.blocks[0].insns[1].what.as_ref().unwrap();
    assert_eq!(what.sources, vec![Loc::Held(held(9)), Loc::Mem(cell(1, 8))]);
    let first = folded(&body(vec![copy(0, 9, 8), load(1, 2, 1, 8), add(2, 3, 2, 9, "add")]));
    assert_eq!(names(&first), ["mov", "add"]);
    let what = first.blocks[0].insns[1].what.as_ref().unwrap();
    assert_eq!(what.sources, vec![Loc::Held(held(9)), Loc::Mem(cell(1, 8))]);
    assert_eq!(first.blocks[0].insns[1].uses, vec![9, 1]);
}

#[test]
fn test_a_load_as_the_first_operand_of_a_subtraction_stays_a_register() {
    let kept = folded(&body(vec![copy(0, 9, 8), load(1, 2, 1, 8), add(2, 3, 2, 9, "sub")]));
    assert_eq!(names(&kept), ["mov", "mov", "sub"]);
    let folded_second = folded(&body(vec![copy(0, 9, 8), load(1, 2, 1, 8), add(2, 3, 9, 2, "sub")]));
    assert_eq!(names(&folded_second), ["mov", "sub"]);
}

#[test]
fn test_a_load_read_twice_or_past_a_store_is_not_an_operand() {
    let twice = folded(&body(vec![copy(0, 9, 8), load(1, 2, 1, 8), add(2, 3, 9, 2, "add"), add(3, 4, 3, 2, "add")]));
    assert_eq!(names(&twice), ["mov", "mov", "add", "add"]);
    let store =
        insn(2, Operation::Move, "mov", vec![Loc::Mem(cell(7, 0))], vec![Loc::Held(held(8))], vec![], vec![7, 8]);
    let blocked = folded(&body(vec![copy(0, 9, 8), load(1, 2, 1, 8), store, add(3, 3, 9, 2, "add")]));
    assert_eq!(names(&blocked), ["mov", "mov", "mov", "add"]);
}

/// The other operand lives on (made in another block, here a loop's invariant):
/// the pin unfolds the load in its two-address pass, `mov t,[m]; add t,r`, and
/// folding it first cost x_transpose +7.6% clocks.
#[test]
fn test_a_load_is_not_an_operand_where_the_other_operand_is_not_made_here() {
    let kept = folded(&body(vec![load(1, 2, 1, 8), add(2, 3, 9, 2, "add")]));
    assert_eq!(names(&kept), ["mov", "add"]);
}
