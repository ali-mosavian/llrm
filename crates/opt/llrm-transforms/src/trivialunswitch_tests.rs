//! Each body is MIR text run by llrm-mir's interpreter before and after, so a wrong loop shows as a wrong
//! answer; what is asserted of the text is where the invariant branch is.

use llrm_analysis::testing::DOS;

use super::TrivialUnswitch;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn unswitched(text: &str, inputs: &[&[i128]]) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, TrivialUnswitch);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

/// The loop of `paths` after tail recursion: `n == 0` is tested in the header on every trip, `m` is the counter.
const HEADER: &str = "define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %body ]
  br i1 %z, label %out, label %test

test:
  %done = icmp eq i16 %i, 0
  br i1 %done, label %out, label %body

body:
  %next = sub i16 %i, 1
  %sum = add i16 %acc, %i
  br label %head

out:
  %r = phi i16 [ %acc, %head ], [ %acc, %test ]
  ret i16 %r
}
";

const INPUTS: &[&[i128]] = &[&[0, 5], &[1, 0], &[1, 5], &[3, 9]];

/// `n == 0` was made at the head of every trip: tail-recursive `paths` ran `sete; mov [esp+14], al` once and
/// `cmp byte [esp+14], 0; je` on each of its trips (rectwo 2212207 clocks, gcc 1191185).
#[test]
fn an_invariant_exit_test_in_the_header_is_made_before_the_loop() {
    let after = unswitched(HEADER, INPUTS);
    let head = after.split("head:").nth(1).expect("the header").split("test:").next().expect("its end");
    assert!(!head.contains("br i1 %z"), "the header still tests it: {after}");
    let entry = after.split("b0:").nth(1).expect("the entry").split("head:").next().expect("its end");
    assert!(entry.contains("br i1 %z, label %out, label %head"), "the entry does not: {after}");
}

/// A condition the loop computes changes with its trips.
#[test]
fn a_condition_the_loop_computes_stays_where_it_is() {
    let text = HEADER.replace("br i1 %z, label %out, label %test", "%z2 = icmp eq i16 %i, 7\n  br i1 %z2, label %out, label %test");
    let after = unswitched(&text, INPUTS);
    assert!(after.contains("br i1 %z2, label %out, label %test"), "{after}");
}

/// Both sides in the loop is an unswitch that copies it (unswitch.rs), not this.
#[test]
fn a_branch_that_stays_in_the_loop_on_both_sides_is_not_moved() {
    let text = HEADER.replace("br i1 %z, label %out, label %test", "br i1 %z, label %body, label %test").replace("[ %acc, %head ], [ %acc, %test ]", "[ %acc, %test ]");
    // With `n == 0` the loop would never end: only the inputs it ends on.
    let after = unswitched(&text, &[&[1, 0], &[1, 5], &[3, 9]]);
    assert!(after.contains("br i1 %z, label %body, label %test"), "{after}");
}

/// What the header does before the branch is skipped where the loop is not entered: a store is not skipped.
#[test]
fn a_header_that_stores_before_its_branch_keeps_its_branch() {
    let text = HEADER.replace("define i16 @f(i16 %n, i16 %m) {", "@g = global i16 0\ndefine i16 @f(i16 %n, i16 %m) {").replace("  br i1 %z, label %out, label %test\n\ntest:", "  store i16 %i, ptr @g\n  br i1 %z, label %out, label %test\n\ntest:");
    let after = unswitched(&text, INPUTS);
    assert!(after.contains("br i1 %z, label %out, label %test"), "{after}");
}

/// A value the loop computes cannot reach the exit on the new edge: the exit's phi keeps it.
#[test]
fn an_exit_value_the_loop_computes_keeps_the_branch() {
    let text = HEADER.replace("[ %acc, %head ], [ %acc, %test ]", "[ %sum0, %head ], [ %acc, %test ]").replace("  br i1 %z, label %out, label %test", "  %sum0 = add i16 %acc, 100\n  br i1 %z, label %out, label %test");
    let after = unswitched(&text, INPUTS);
    assert!(after.contains("br i1 %z, label %out, label %test"), "{after}");
}
