//! Adapted from llrm-codegen's `optimize/lcssa_tests.rs`, the port of
//! `tests/test_lcssa.py`, each body now MIR text; `==` on bodies is
//! printed text.

use llrm_mir::passes::PassManager;

use super::{LoopClosedSSA, closed};
use crate::testing::{f, parsed, printed, results};

/// A counted loop whose carried value is read after it.
pub const LOOP_WITH_EXIT_USE: &str = "define i16 @f(i16 %seed, i16 %n) {
b0:
  br label %b1

b1:
  %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b2 ]
  %more = icmp ult i16 %carried, %n
  br i1 %more, label %b2, label %b3

b2:
  %stepped = add i16 %carried, 1
  br label %b1

b3:
  %answer = add i16 %carried, 7
  ret i16 %answer
}
";

/// `text` closed, printed; and closed again, unchanged.
fn closed_once(text: &str) -> String {
    let mut module = parsed(text);
    closed(f(&mut module)).unwrap();
    let once = printed(&module);
    assert!(!closed(f(&mut module)).unwrap(), "closing is idempotent");
    assert_eq!(printed(&module), once);
    once
}

/// `text`, which need not verify, left as it is.
fn untouched(text: &str) {
    let mut module = parsed(text);
    let before = llrm_mir::print::module(&module);
    assert!(!closed(f(&mut module)).unwrap());
    assert_eq!(llrm_mir::print::module(&module), before);
}

#[test]
fn test_a_loop_value_used_after_the_exit_gets_an_exit_phi() {
    assert_eq!(
        closed_once(LOOP_WITH_EXIT_USE),
        "define i16 @f(i16 %seed, i16 %n) {
b0:
  br label %b1

b1:
  %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b2 ]
  %more = icmp ult i16 %carried, %n
  br i1 %more, label %b2, label %b3

b2:
  %stepped = add i16 %carried, 1
  br label %b1

b3:
  %carried.lcssa = phi i16 [ %carried, %b1 ]
  %answer = add i16 %carried.lcssa, 7
  ret i16 %answer
}
"
    );
}

#[test]
fn test_loop_closed_ssa_is_idempotent() {
    closed_once(LOOP_WITH_EXIT_USE);
}

#[test]
fn test_exit_edge_into_a_bypass_join_is_closed_once() {
    assert_eq!(
        closed_once(
            "define i16 @f(i16 %seed, i16 %n, i1 %c) {
b0:
  br i1 %c, label %b1, label %b4

b1:
  %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b2 ]
  %more = icmp ult i16 %carried, %n
  br i1 %more, label %b2, label %b3

b2:
  %stepped = add i16 %carried, 1
  br label %b1

b3:
  br label %b4

b4:
  %answer = phi i16 [ %seed, %b0 ], [ %carried, %b3 ]
  ret i16 %answer
}
"
        ),
        "define i16 @f(i16 %seed, i16 %n, i1 %c) {
b0:
  br i1 %c, label %b1, label %b4

b1:
  %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b2 ]
  %more = icmp ult i16 %carried, %n
  br i1 %more, label %b2, label %b3

b2:
  %stepped = add i16 %carried, 1
  br label %b1

b3:
  %carried.lcssa = phi i16 [ %carried, %b1 ]
  br label %b4

b4:
  %answer = phi i16 [ %seed, %b0 ], [ %carried.lcssa, %b3 ]
  ret i16 %answer
}
"
    );
}

#[test]
fn test_a_value_already_consumed_by_an_exit_phi_is_closed() {
    untouched(&LOOP_WITH_EXIT_USE.replace(
        "  %answer = add i16 %carried, 7\n  ret i16 %answer",
        "  %result = phi i16 [ %carried, %b1 ]\n  ret i16 %result",
    ));
}

#[test]
fn test_multiple_edges_to_one_dedicated_exit_are_closed() {
    let text = LOOP_WITH_EXIT_USE.replace(
        "  %stepped = add i16 %carried, 1\n  br label %b1",
        "  %stepped = add i16 %carried, 1\n  %small = icmp ult i16 %stepped, 50\n  br i1 %small, label %b1, label %b3",
    );
    assert!(closed_once(&text).contains(
        "b3:
  %carried.lcssa = phi i16 [ %carried, %b1 ], [ %carried, %b2 ]
  %answer = add i16 %carried.lcssa, 7
"
    ));
}

/// qcport keybind: a switch with two cases to the exit got an exit phi with one
/// input for its two edges, which the verifier refused.
#[test]
fn test_a_switch_with_two_cases_to_the_exit_gives_the_exit_phi_two_inputs() {
    let text = LOOP_WITH_EXIT_USE.replace(
        "  %stepped = add i16 %carried, 1\n  br label %b1",
        "  %stepped = add i16 %carried, 1\n  switch i16 %stepped, label %b1 [\n    i16 5, label %b3\n    i16 9, label %b3\n  ]",
    );
    assert!(
        closed_once(&text).contains("%carried.lcssa = phi i16 [ %carried, %b1 ], [ %carried, %b2 ], [ %carried, %b2 ]")
    );
}

/// Not SSA in either MIR: `%stepped` does not reach the exit from `%b1`.
#[test]
fn test_exit_phi_cannot_read_a_value_missing_on_one_edge() {
    untouched(
        &LOOP_WITH_EXIT_USE
            .replace("  br label %b1\n\nb3:", "  br i1 %more, label %b1, label %b3\n\nb3:")
            .replace("%answer = add i16 %carried, 7", "%answer = add i16 %stepped, 7"),
    );
}

/// Not SSA either: `%carried` does not reach the exit from `%b0`.
#[test]
fn test_exit_shared_with_a_bypass_still_requires_canonicalization() {
    untouched(
        &LOOP_WITH_EXIT_USE
            .replace("b0:\n  br label %b1", "b0:\n  %c = icmp eq i16 %n, 0\n  br i1 %c, label %b1, label %b3"),
    );
}

/// LLVM's formLCSSA only adds phis; the one that names a single value is
/// InstCombine's to remove. `closed` removed it, as did llrm's LCSSA before
/// every loop pass: a step the pin's pass does not have.
#[test]
fn test_closing_a_loop_leaves_a_phi_naming_one_value_outside_its_exits() {
    let text = "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %joined = phi i16 [ %x, %b1 ], [ %x, %b2 ]
  br label %b4

b4:
  %copied = phi i16 [ %joined, %b3 ]
  %read = add i16 %copied, 1
  ret i16 %read
}
";
    assert_eq!(closed_once(text), text);
}

/// An inner loop closes first, so the outer loop's exit reads the inner
/// exit's phi; what the function returns is unchanged.
#[test]
fn test_nested_loops_close_inner_first_and_keep_their_results() {
    let text = "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b3 ]
  br label %b2

b2:
  %j = phi i16 [ 0, %b1 ], [ %j.next, %b2 ]
  %j.next = add i16 %j, 1
  %inner = icmp ult i16 %j, %i
  br i1 %inner, label %b2, label %b3

b3:
  %i.next = add i16 %i, 1
  %outer = icmp ult i16 %i.next, %n
  br i1 %outer, label %b1, label %b4

b4:
  %r = add i16 %j.next, %i.next
  ret i16 %r
}
";
    let inputs: &[&[i128]] = &[&[0], &[1], &[5]];
    let expected = results(&parsed(text), inputs);
    let mut module = parsed(text);
    closed(f(&mut module)).unwrap();
    let text = printed(&module);
    assert!(text.contains("b3:\n  %j.next.lcssa = phi i16 [ %j.next, %b2 ]\n  %i.next = add i16 %i, 1\n"), "{text}");
    assert!(
        text.contains("b4:\n  %i.next.lcssa = phi i16 [ %i.next, %b3 ]\n  %j.next.lcssa.lcssa = phi i16 [ %j.next.lcssa, %b3 ]\n  %r = add i16 %j.next.lcssa.lcssa, %i.next.lcssa\n"),
        "{text}"
    );
    assert_eq!(results(&module, inputs), expected);
}

/// A value read only inside its loop needs no exit phi.
#[test]
fn test_a_value_used_only_inside_its_loop_is_left_alone() {
    untouched(&LOOP_WITH_EXIT_USE.replace("%answer = add i16 %carried, 7", "%answer = add i16 %n, 7"));
}

/// Through the pass manager, verified, the function returns what it did.
#[test]
fn test_the_pass_keeps_what_the_function_returns() {
    let inputs: &[&[i128]] = &[&[0, 0], &[3, 10], &[12, 5]];
    let expected = results(&parsed(LOOP_WITH_EXIT_USE), inputs);
    let mut module = parsed(LOOP_WITH_EXIT_USE);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add(LoopClosedSSA);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    assert!(printed(&module).contains("%carried.lcssa = phi"));
    assert_eq!(results(&module, inputs), expected);
}

#[test]
fn test_only_the_values_read_outside_get_exit_phis() {
    let closed = closed_once(LOOP_WITH_EXIT_USE);
    assert_eq!(closed.matches(".lcssa = phi").count(), 1, "{closed}");
    assert!(closed.contains("%carried.lcssa = phi i16 [ %carried, %b1 ]"), "{closed}");
}

#[test]
fn test_closing_over_a_held_shape_is_closing() {
    let mut module = parsed(LOOP_WITH_EXIT_USE);
    let shape = llrm_analysis::cfg::Shape::of(f(&mut module));
    assert!(super::closed_in(f(&mut module), &shape).unwrap());
    assert_eq!(printed(&module), closed_once(LOOP_WITH_EXIT_USE));
}

/// A loop with two exits that meet at `merge`, then a branch that joins again:
/// closing made a phi at every join below the first, `%s.lcssa3` and
/// `%s.lcssa4` of one value, which the next `closed` took away again
/// (`closed` was not a fixed point: 9 QCport modules, `LcssaForm` check).
/// LLVM's SSAUpdater makes a phi only where the values differ.
#[test]
fn test_two_exits_that_meet_make_one_merge_phi_not_one_per_join_below() {
    let closed = closed_once(
        "declare i16 @g(i16)

define i16 @f(i16 %n, i16 %m) {
b1:
  br label %h

h:
  %i = phi i16 [ 0, %b1 ], [ %i2, %latch ]
  %s = phi i16 [ 0, %b1 ], [ %s2, %latch ]
  %c = icmp ult i16 %i, %n
  br i1 %c, label %body, label %x1

body:
  %z = icmp eq i16 %i, %m
  br i1 %z, label %x2, label %latch

latch:
  %i2 = add i16 %i, 1
  %s2 = add i16 %s, %i
  br label %h

x1:
  br label %merge

x2:
  br label %merge

merge:
  %t = icmp eq i16 %m, 0
  br i1 %t, label %j, label %k

k:
  %r = call i16 @g(i16 %m)
  br label %j

j:
  %e = icmp eq i16 %n, 3
  br i1 %e, label %b27, label %b26

b27:
  br label %b26

b26:
  %out = add i16 %s, 1
  ret i16 %out
}
",
    );
    assert_eq!(
        closed.lines().filter(|line| line.contains(".lcssa") && line.contains(" = phi ")).count(),
        3,
        "{closed}"
    );
    assert!(closed.contains("add i16 %s.lcssa2, 1"), "{closed}");
}

/// `closed` found each loop's exits twice (once to know which blocks are exits,
/// once to close the loop) and a multi-exit loop built its own dominators,
/// graph and predecessors: 2.47 M against the pin's 0.28 M on d_faces' six
/// functions. The edges leaving a loop are found once per closing.
#[test]
fn test_closing_finds_the_exits_of_a_loop_once() {
    use llrm_mir::counts::{Event, get};
    let mut module = parsed(LOOP_WITH_EXIT_USE);
    let before = get(Event::LoopExitScans);
    closed(f(&mut module)).unwrap();
    assert_eq!(get(Event::LoopExitScans) - before, 1);
}
