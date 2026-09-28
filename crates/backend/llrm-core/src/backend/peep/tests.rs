//! The engine features the argument rules need: matching a held value's
//! definition before the window, and the caller's read counts.

use std::sync::Arc;

use iced_x86::Register;

use super::walk::Facts;
use crate::model::ir::{Addr, Held, Loc, Mem, Operation, Semantics, Space};
use crate::model::lir::Insn;
use crate::support::hash::IndexMap;

fn what(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

fn argument() -> Mem {
    Mem { through: Register::BP, ..Mem::new(Some(Addr::new(Space::Frame, 6)), 2) }
}

/// `mov v,[bp+6]`, `between`, `push v`; what memory_arguments makes of it
/// when the caller counted v read `reads` times.
fn folded(between: Semantics, reads: i64) -> Vec<Arc<Insn>> {
    let value = Held { value: 1, width: 2 };
    let load = Insn::new(0, Some((0, 0)), Some(what(Operation::Move, "mov", vec![Loc::Held(value)], vec![Loc::Mem(argument())])), vec![1], vec![]);
    let crossed = Insn::new(1, Some((1, 1)), Some(between), vec![2], vec![]);
    let push = Insn::new(2, Some((2, 2)), Some(what(Operation::Push, "push", vec![], vec![Loc::Held(value)])), vec![], vec![1]);
    let counts: IndexMap<u32, i64> = IndexMap::from_iter([(1, reads)]);
    super::memory_arguments_insns(&[Arc::new(load), Arc::new(crossed), Arc::new(push)], &Facts::counted(&counts))
}

fn pushes(insns: &[Arc<Insn>]) -> Vec<Loc> {
    insns.iter().filter_map(|one| one.what.as_ref()).filter(|what| what.op == Operation::Push).map(|what| what.sources[0].clone()).collect()
}

/// A rule matching a one-instruction window panicked in the window walk
/// (a slice starting past its end) on every procedure pushing a loaded
/// argument; tests/suite/procs.bas stopped compiling.
#[test]
fn a_load_read_once_is_pushed_from_its_cell_across_unrelated_work() {
    let other = Held { value: 2, width: 2 };
    let out = folded(what(Operation::Move, "mov", vec![Loc::Held(other)], vec![Loc::Held(other)]), 1);
    assert_eq!(pushes(&out), [Loc::Mem(argument())]);
    assert_eq!(out.len(), 2, "the load leaves the block");
}

/// The definition's cell may change before the push: a store between keeps
/// the push reading the loaded value.
#[test]
fn a_store_between_the_load_and_its_push_keeps_the_load() {
    let out = folded(what(Operation::Move, "mov", vec![Loc::Mem(argument())], vec![Loc::Held(Held { value: 2, width: 2 })]), 1);
    assert_eq!(pushes(&out), [Loc::Held(Held { value: 1, width: 2 })]);
}

/// A value read twice must stay loaded for its other reader.
#[test]
fn a_load_read_twice_is_not_pushed_from_its_cell() {
    let other = Held { value: 2, width: 2 };
    let out = folded(what(Operation::Move, "mov", vec![Loc::Held(other)], vec![Loc::Held(other)]), 2);
    assert_eq!(pushes(&out), [Loc::Held(Held { value: 1, width: 2 })]);
}

/// The old pass dropped every definition of a folded value, the rule drops
/// the one it matched: they agree only while a held value has one
/// definition, so a second one stops the walk rather than diverging.
#[test]
#[should_panic(expected = "held value 1 is defined 2 times")]
fn a_folded_value_defined_twice_is_refused() {
    let value = Held { value: 1, width: 2 };
    let load = |at| Arc::new(Insn::new(at, Some((at, at)), Some(what(Operation::Move, "mov", vec![Loc::Held(value)], vec![Loc::Mem(argument())])), vec![1], vec![]));
    let push = Arc::new(Insn::new(1, Some((1, 1)), Some(what(Operation::Push, "push", vec![], vec![Loc::Held(value)])), vec![], vec![1]));
    let counts: IndexMap<u32, i64> = IndexMap::from_iter([(1, 1)]);
    super::memory_arguments_insns(&[load(0), push, load(2)], &Facts::counted(&counts));
}
