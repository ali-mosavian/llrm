//! Each body is MIR text in loop-simplify and LCSSA form, run by llrm-mir's
//! interpreter before and after, so a wrong deletion shows as a wrong answer.

use super::LoopDeletion;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn deleted(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let before = parsed(text);
    let mut module = before.clone();
    let after = managed(&mut module, LoopDeletion);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

const INPUTS: &[&[i128]] = &[&[0, 5], &[1, 0], &[3, 9]];

/// A loop that only counts, tested in its header: `x` is the answer, the
/// counter is read by nobody.
const COUNTER: &str = "define i16 @f(i16 %x, i16 %n) mustprogress {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %body ]
  %go = icmp slt i16 %i, 10
  br i1 %go, label %body, label %out

body:
  %next = add i16 %i, 1
  br label %head

out:
  ret i16 %x
}
";

#[test]
fn a_loop_that_only_counts_is_deleted() {
    let after = deleted(COUNTER, INPUTS);
    assert!(!after.contains("phi"), "{after}");
    assert!(after.contains("br label %out"), "{after}");
}

/// The same loop rotated: one block, tested at the bottom. The old pass took
/// only the header-tested form (`loopexit.rs` took two blocks with the test
/// before the step), so a rotated counter loop stayed.
#[test]
fn a_rotated_loop_that_only_counts_is_deleted() {
    let text = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %loop

loop:
  %i = phi i16 [ 0, %b0 ], [ %next, %loop ]
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 10
  br i1 %go, label %loop, label %out

out:
  ret i16 %x
}
";
    let after = deleted(text, INPUTS);
    assert!(!after.contains("phi"), "{after}");
}

/// A value the exit phi takes that the loop computes is not invariant.
#[test]
fn a_loop_whose_result_is_read_stays() {
    let text = COUNTER.replace("out:\n  ret i16 %x", "out:\n  %r = phi i16 [ %i, %head ]\n  ret i16 %r");
    let after = deleted(&text, INPUTS);
    assert!(after.contains("phi i16 [ 0"), "{after}");
}

#[test]
fn a_loop_that_stores_stays() {
    let text = "@g = global i16 0

define i16 @f(i16 %x, i16 %n) mustprogress {
b0:
  br label %loop

loop:
  %i = phi i16 [ 0, %b0 ], [ %next, %loop ]
  store i16 %i, ptr @g
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 10
  br i1 %go, label %loop, label %out

out:
  ret i16 %x
}
";
    let after = deleted(text, INPUTS);
    assert!(after.contains("store i16 %i"), "{after}");
}

/// A loop nothing may end (it waits on memory) is kept unless it is told to
/// make progress. (`i < n` has a bound ScalarEvolution finds, so it is not
/// such a loop.)
#[test]
fn a_loop_of_unknown_length_stays_unless_it_must_make_progress() {
    let waiting = "@g = global i16 7

define i16 @f(i16 %x, i16 %n) {
b0:
  br label %loop

loop:
  %v = load i16, ptr @g
  %go = icmp ne i16 %v, 7
  br i1 %go, label %loop, label %out

out:
  ret i16 %x
}
";
    let after = deleted(waiting, &[&[1, 0]]);
    assert!(after.contains("load"), "{after}");
    let told = waiting.replace("@f(i16 %x, i16 %n) {", "@f(i16 %x, i16 %n) mustprogress {");
    let after = deleted(&told, &[&[1, 0]]);
    assert!(!after.contains("load"), "{after}");
}

/// A loop its preheader's only predecessor never enters is deleted, whatever
/// it holds.
#[test]
fn a_loop_that_never_executes_is_deleted() {
    let text = "@g = global i16 0

define i16 @f(i16 %x, i16 %n) {
b0:
  br i1 false, label %pre, label %out

pre:
  br label %loop

loop:
  %i = phi i16 [ 0, %pre ], [ %next, %loop ]
  store i16 %i, ptr @g
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, %n
  br i1 %go, label %loop, label %exit

exit:
  br label %out

out:
  %r = phi i16 [ %x, %b0 ], [ %x, %exit ]
  ret i16 %r
}
";
    let after = deleted(text, INPUTS);
    assert!(!after.contains("store"), "{after}");
}

/// A loop that stores but leaves on its first iteration has no backedge left.
#[test]
fn a_loop_that_exits_on_its_first_iteration_loses_its_backedge() {
    let text = "@g = global i16 0

define i16 @f(i16 %x, i16 %n) {
b0:
  br label %loop

loop:
  %i = phi i16 [ 0, %b0 ], [ %next, %latch ]
  store i16 %x, ptr @g
  %c = icmp eq i16 %i, 0
  br i1 %c, label %out, label %latch

latch:
  %next = add i16 %i, 1
  br label %loop

out:
  ret i16 %x
}
";
    let after = deleted(text, INPUTS);
    assert!(after.contains("unreachable"), "the latch no longer jumps back: {after}");
}

/// `n` dead loops in a row cost entries in proportion to `n`.
fn sequence(n: usize) -> String {
    let mut text = String::from("define i16 @f(i16 %x) mustprogress {\nb0:\n  br label %l0\n\n");
    for k in 0..n {
        let next = if k + 1 == n { "end".to_owned() } else { format!("l{}", k + 1) };
        let pre = if k == 0 { "b0".to_owned() } else { format!("x{}", k - 1) };
        text += &format!(
            "l{k}:\n  %i{k} = phi i16 [ 0, %{pre} ], [ %n{k}, %l{k} ]\n  %n{k} = add i16 %i{k}, 1\n  %g{k} = icmp slt i16 %n{k}, 10\n  br i1 %g{k}, label %l{k}, label %x{k}\n\nx{k}:\n  br label %{next}\n\n"
        );
    }
    text += "end:\n  ret i16 %x\n}\n";
    text
}

#[test]
fn deleting_loops_in_sequence_scans_in_proportion_to_their_number() {
    use llrm_mir::edit::{instructions_scanned, layout_scanned, uses_scanned};
    let scanned = |n: usize| {
        let mut module = parsed(&sequence(n));
        let before = layout_scanned() + instructions_scanned() + uses_scanned();
        let after = managed(&mut module, LoopDeletion);
        assert!(!after.contains("phi"), "{after}");
        layout_scanned() + instructions_scanned() + uses_scanned() - before
    };
    let (a, b, c) = (scanned(64), scanned(128), scanned(256));
    assert!(b * 100 <= a * 250 && c * 100 <= b * 250, "64: {a}, 128: {b}, 256: {c}");
}
