use std::sync::Arc;

use super::{stack_to_add, Bound, Usage, STACK_RESERVE};
use crate::backend::masm::{Callee, Module, Procedure};
use crate::model::ir::{self, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

fn insn(at: i64, op: Operation, name: &str, sources: Vec<Loc>) -> Arc<Insn> {
    Arc::new(Insn::new(at, Some((at, at)), Some(Semantics { name: Some(name.to_owned()), sources, ..Semantics::new(op) }), vec![], vec![]))
}

fn push(at: i64) -> Arc<Insn> {
    insn(at, Operation::Push, "push", vec![Loc::Imm(ir::Imm { value: 1, width: 2, address: None })])
}

/// A far procedure: `pushes` words pushed for a call, `reserve` bytes of locals, and its callees.
fn procedure(name: &str, reserve: i64, pushes: usize, callees: &[&str]) -> Procedure {
    let mut insns = (0..pushes as i64).map(push).collect::<Vec<_>>();
    insns.push(insn(100, Operation::Call, "call", vec![]));
    insns.push(insn(101, Operation::Return, "ret", vec![]));
    let body = LirBody::new(name, 0, vec![LirBlock::new(0, insns)], IndexMap::default(), IndexMap::default());
    let callees = callees.iter().enumerate().map(|(at, one)| (100 + at as i64, Callee::new(*one, true))).collect();
    Procedure { name: name.to_owned(), public: true, far: true, body, reserve, callees, interrupt: None, size: false, entry: 0 }
}

fn module(procedures: Vec<Procedure>) -> Module {
    Module { code: String::new(), names: IndexMap::default(), externs: Vec::new(), publics: Vec::new(), data: Vec::new(), procedures, private: Default::default(), requests: Default::default(), debug: None, stack: 0 }
}

/// What a program can use of the stack was nowhere stated: a deep chain of
/// frames overflowed a DOS stack, and nothing said it could. A frame is its
/// return address, BP and locals, and what a call has pushed.
#[test]
fn test_the_stack_a_chain_of_calls_can_reach_is_the_sum_of_its_frames() {
    let usage = Usage::of(&[module(vec![procedure("main", 10, 3, &["f"]), procedure("f", 20, 0, &["g"]), procedure("g", 0, 0, &[])])]);
    // far return 4 + push bp 2 + locals 10 + 3 words pushed: 22; f: 4 + 2 + 20; g: 4.
    assert_eq!(usage.bound("g"), Bound::Bytes(4));
    assert_eq!(usage.bound("f"), Bound::Bytes(26 + 4));
    assert_eq!(usage.bound("main"), Bound::Bytes(22 + 26 + 4));
    assert_eq!(usage.roots(), ["main"]);
    assert_eq!(usage.warnings(100), Vec::<String>::new());
    assert_eq!(usage.warnings(40).len(), 1, "{:?}", usage.warnings(40));
}

/// A cycle has no bound, and a routine the program does not define adds what
/// nothing here can know: the bound is a floor.
#[test]
fn test_a_cycle_is_unbounded_and_an_undefined_callee_makes_the_bound_a_floor() {
    let usage = Usage::of(&[module(vec![procedure("ping", 0, 0, &["pong"]), procedure("pong", 0, 0, &["ping"]), procedure("shell", 0, 0, &["B$PRINT"])])]);
    assert_eq!(usage.bound("ping"), Bound::Recursive);
    assert!(matches!(usage.bound("shell"), Bound::AtLeast(4, ref named) if named.contains("B$PRINT")), "{:?}", usage.bound("shell"));
    assert!(usage.report().contains("unbounded (recursion)") && usage.report().contains(">= 4 (and B$PRINT)"));
}

/// Every runtime helper is a callee shared by many: walking each path
/// took time exponential in a chain of diamonds, 2^30 paths here. Each bound
/// is its frame and the deepest of its callees', memoized.
#[test]
fn test_a_chain_of_diamonds_thirty_deep_is_settled_at_once() {
    let mut procedures = Vec::new();
    for level in 0..30 {
        let (next_left, next_right) = (format!("l{}", level + 1), format!("r{}", level + 1));
        let callees: Vec<&str> = if level == 29 { vec!["end"] } else { vec![next_left.as_str(), next_right.as_str()] };
        procedures.push(procedure(&format!("l{level}"), 2, 0, &callees));
        procedures.push(procedure(&format!("r{level}"), 2, 0, &callees));
    }
    procedures.push(procedure("end", 0, 0, &[]));
    procedures.push(procedure("top", 0, 0, &["l0", "r0"]));
    let usage = Usage::of(&[module(procedures)]);
    // Each level: far return 4 + push bp 2 + 2 locals = 8; the end and the top, with no frame, 4.
    assert_eq!(usage.bound("top"), Bound::Bytes(4 + 30 * 8 + 4));
}

/// `enter N,0` pushes BP and takes the locals as the three instructions it replaces did: the
/// bound must not lose them (a frame read only `push` and `sub` and counted none).
#[test]
fn test_a_frame_opened_with_enter_counts_its_bp_and_locals() {
    let usage = |enter: bool| Usage::of(&[module(vec![Procedure { size: enter, ..procedure("main", 10, 3, &[]) }])]);
    assert_eq!(usage(true).bound("main"), usage(false).bound("main"));
    assert_eq!(usage(true).bound("main"), Bound::Bytes(4 + 2 + 10 + 6));
}


/// #396: a SUB's locals live in the runtime's frame (`mov cx,N` / `call B$ENSA`), which no
/// instruction of it shows; its bound was the return address alone, so a 6000-byte frame sized
/// no stack and crashed on entry.
#[test]
fn test_a_runtime_frame_counts_the_locals_its_entry_call_takes() {
    let usage = Usage::of(&[module(vec![Procedure { entry: 6010, ..procedure("big", 0, 0, &[]) }])]);
    assert_eq!(usage.bound("big"), Bound::Bytes(4 + 6010));
}

/// #396: the stack a chain needs beyond the runtime's own, none for a chain that fits it.
#[test]
fn test_the_stack_to_add_is_what_the_chain_needs_beyond_the_base() {
    let one = |entry| module(vec![Procedure { entry, ..procedure("big", 0, 0, &[]) }]);
    assert_eq!(stack_to_add(&one(100), 0x800), Ok(0));
    assert_eq!(stack_to_add(&one(6010), 0x800), Ok(4 + 6010 + STACK_RESERVE - 0x800));
    assert!(stack_to_add(&one(0xF000), 0x800).unwrap_err().contains("bytes of stack"));
}
