//! The engine features the argument rules need: matching a held value's
//! definition before the window, and the caller's read counts.

use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::walk::Facts;
use crate::model::ir::{Addr, Held, Loc, Mem, Operation, Semantics, Space};
use crate::model::lir::Insn;
use crate::support::hash::IndexMap;

fn what(
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Semantics {
    Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
}

fn argument() -> Mem {
    Mem { through: RegId::BP, ..Mem::new(Some(Addr::new(Space::Frame, 6)), 2) }
}

/// `mov v,[bp+6]`, `between`, `push v`; what memory_arguments makes of it
/// when the caller counted v read `reads` times.
fn folded(
    between: Semantics,
    reads: i64,
) -> Vec<Arc<Insn>> {
    let value = Held { value: 1, width: 2 };
    let load = Insn::new(
        0,
        Some((0, 0)),
        Some(what(Operation::Move, "mov", vec![Loc::Held(value)], vec![Loc::Mem(argument())])),
        vec![1],
        vec![],
    );
    let crossed = Insn::new(1, Some((1, 1)), Some(between), vec![2], vec![]);
    let push = Insn::new(
        2,
        Some((2, 2)),
        Some(what(Operation::Push, "push", vec![], vec![Loc::Held(value)])),
        vec![],
        vec![1],
    );
    let counts: IndexMap<u32, i64> = IndexMap::from_iter([(1, reads)]);
    super::rewritten_insns(
        crate::backend::targets::x86_m16::RULES.memory_arguments,
        &[Arc::new(load), Arc::new(crossed), Arc::new(push)],
        &Facts::counted(&counts, 16, crate::backend::registerinfo::test_regs()),
    )
}

fn pushes(insns: &[Arc<Insn>]) -> Vec<Loc> {
    insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .filter(|what| what.op == Operation::Push)
        .map(|what| what.sources[0].clone())
        .collect()
}

/// A rule matching a one-instruction window panicked in the window walk
/// (a slice starting past its end) on every procedure pushing a loaded
/// argument; tests/run/qb/procs.bas stopped compiling.
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
    let out = folded(
        what(Operation::Move, "mov", vec![Loc::Mem(argument())], vec![Loc::Held(Held { value: 2, width: 2 })]),
        1,
    );
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
    let load = |at| {
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(what(Operation::Move, "mov", vec![Loc::Held(value)], vec![Loc::Mem(argument())])),
            vec![1],
            vec![],
        ))
    };
    let push = Arc::new(Insn::new(
        1,
        Some((1, 1)),
        Some(what(Operation::Push, "push", vec![], vec![Loc::Held(value)])),
        vec![],
        vec![1],
    ));
    let counts: IndexMap<u32, i64> = IndexMap::from_iter([(1, 1)]);
    super::rewritten_insns(
        crate::backend::targets::x86_m16::RULES.memory_arguments,
        &[load(0), push, load(2)],
        &Facts::counted(&counts, 16, crate::backend::registerinfo::test_regs()),
    );
}

/// A target with no `peephole.peep` has no rules, not m16's: its code is
/// left as the selector made it.
#[test]
fn a_target_without_rules_leaves_the_code_alone() {
    let value = Held { value: 1, width: 2 };
    let load = Arc::new(Insn::new(
        0,
        Some((0, 0)),
        Some(what(Operation::Move, "mov", vec![Loc::Held(value)], vec![Loc::Mem(argument())])),
        vec![1],
        vec![],
    ));
    let push = Arc::new(Insn::new(
        1,
        Some((1, 1)),
        Some(what(Operation::Push, "push", vec![], vec![Loc::Held(value)])),
        vec![],
        vec![1],
    ));
    let counts: IndexMap<u32, i64> = IndexMap::from_iter([(1, 1)]);
    let facts = Facts::counted(&counts, 16, crate::backend::registerinfo::test_regs());
    let insns = [load, push];
    assert_eq!(super::rewritten_insns(None, &insns, &facts).len(), 2);
    assert_eq!(super::rewritten_insns(super::Rules::NONE.memory_arguments, &insns, &facts).len(), 2);
    // m16's rules fold the load into the push.
    assert_eq!(
        super::rewritten_insns(crate::backend::targets::x86_m16::RULES.memory_arguments, &insns, &facts).len(),
        1
    );
}

/// The rules are bound to a target through its selector: the one generated
/// from the same directory.
#[test]
fn a_selector_carries_the_rules_of_its_directory() {
    let selector = crate::backend::isel::selector("x86-m16").expect("m16's selector");
    assert!(std::ptr::eq(selector.rules(), &crate::backend::targets::x86_m16::RULES));
    assert!(selector.rules().far_loads.is_some() && selector.rules().zero_jcc.contains("je"));
}

/// The groups of `groups.list` are the groups the targets have between them and
/// a target with no rules has none: one list names them for the struct, `NONE`
/// and the checks.
#[test]
fn the_rule_groups_come_from_one_list() {
    let mut present = crate::backend::targets::x86_m16::RULES.present();
    present.extend(crate::backend::targets::x86_m32::RULES.present());
    present.sort_unstable();
    present.dedup();
    let mut all = super::Rules::GROUPS.to_vec();
    all.sort_unstable();
    assert_eq!(present, all);
    assert!(super::Rules::NONE.present().is_empty());
    assert_eq!(super::Rules::GROUPS.len(), 26);
}

/// A window was built, and the group's first-pattern test run, at every
/// instruction of a block: 24,604 tests on d_faces -O2, 856 of them passed
/// (3.5%). A group whose rules all start on a `mov` asks `head` only at one.
#[test]
fn test_a_group_asks_its_first_test_only_of_the_operations_its_rules_start_on() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::walk::{self, Cx, Matcher, Rewrite, Skip, Window};

    static ASKED: AtomicUsize = AtomicUsize::new(0);
    fn head(_: &Window) -> u32 {
        ASKED.fetch_add(1, Ordering::Relaxed);
        0
    }
    fn tail(
        _: &Cx,
        _: &Window,
        _: u32,
    ) -> Option<Rewrite> {
        None
    }
    let matcher = Matcher { width: 1, first: &[Operation::Move], head, tail, cross: None };
    let value = Held { value: 1, width: 2 };
    let insns: Vec<Arc<Insn>> =
        [(Operation::Move, "mov"), (Operation::Binary, "add"), (Operation::Push, "push"), (Operation::Binary, "sub")]
            .into_iter()
            .enumerate()
            .map(|(at, (op, name))| {
                Arc::new(Insn::new(
                    at as i64,
                    Some((at as i64, at as i64)),
                    Some(what(op, name, vec![Loc::Held(value)], vec![])),
                    vec![],
                    vec![],
                ))
            })
            .collect();
    let counts: IndexMap<u32, i64> = IndexMap::default();
    walk::window_insns(
        &insns,
        &Facts::counted(&counts, 16, crate::backend::registerinfo::test_regs()),
        &matcher,
        Skip::None,
    );
    assert_eq!(ASKED.load(Ordering::Relaxed), 1);
}

/// Every instruction of a block got a copy of its window's indices before the
/// first-pattern test: d_faces -O2 made 24 K such vectors, 23 K for nothing.
/// A block with no instruction a rule starts on now costs a constant.
#[test]
fn test_a_block_no_rule_starts_in_allocates_nothing_per_instruction() {
    use super::walk::{self, Cx, Matcher, Rewrite, Skip, Window};

    fn head(_: &Window) -> u32 {
        0
    }
    fn tail(
        _: &Cx,
        _: &Window,
        _: u32,
    ) -> Option<Rewrite> {
        None
    }
    let matcher = Matcher { width: 2, first: &[Operation::Move], head, tail, cross: None };
    let insns: Vec<Arc<Insn>> = (0..64)
        .map(|at| {
            Arc::new(Insn::new(
                at,
                Some((at, at)),
                Some(what(Operation::Binary, "add", vec![], vec![])),
                vec![],
                vec![],
            ))
        })
        .collect();
    let counts: IndexMap<u32, i64> = IndexMap::default();
    let facts = Facts::counted(&counts, 16, crate::backend::registerinfo::test_regs());
    let (_, made) = llrm_support::alloc_count::counted(|| walk::window_insns(&insns, &facts, &matcher, Skip::None));
    assert!(made <= 8, "{made} allocations walking 64 instructions no rule starts on");
}
