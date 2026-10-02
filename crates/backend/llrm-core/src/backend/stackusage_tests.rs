use std::sync::Arc;

use super::{Bound, Usage};
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
    Procedure { name: name.to_owned(), public: true, far: true, body, reserve, callees, interrupt: None }
}

fn module(procedures: Vec<Procedure>) -> Module {
    Module { code: String::new(), names: IndexMap::default(), externs: Vec::new(), publics: Vec::new(), data: Vec::new(), procedures, private: Default::default(), requests: Default::default(), debug: None }
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
