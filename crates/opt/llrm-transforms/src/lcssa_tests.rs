//! Adapted from llrm-core's `optimize/lcssa_tests.rs`, the port of
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

/// matmul8: peeling left the outer counter's step behind `phi(x, x)`, and
/// after unrolling an inner loop its exit phi stayed: no trip count through
/// the one, no unrolling past the other, so both loop nests stayed rolled.
#[test]
fn test_a_phi_naming_one_value_outside_a_loop_exit_is_that_value() {
    assert_eq!(
        closed_once(
            "define i16 @f(i16 %x, i1 %c) {
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
"
        ),
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  br label %b4

b4:
  %read = add i16 %x, 1
  ret i16 %read
}
"
    );
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
