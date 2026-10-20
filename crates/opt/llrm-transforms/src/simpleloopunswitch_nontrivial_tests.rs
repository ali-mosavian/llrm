//! Each body is MIR text in loop-simplify and LCSSA form, run by llrm-mir's
//! interpreter before and after, so a wrong copy shows as a wrong answer; what
//! is asserted of the text is how many loops there are and where the test is.

use crate::simpleloopunswitch::SimpleLoopUnswitch;
use crate::testing::{managed, parsed, results};

/// `text` through the pass, `non_trivial` or not: its printed form, run as
/// before on `inputs`.
fn unswitched(
    text: &str,
    inputs: &[&[i128]],
    non_trivial: bool,
) -> String {
    let before = parsed(text);
    let mut module = before.clone();
    let after = managed(&mut module, SimpleLoopUnswitch { non_trivial });
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

/// How many loops `text` has: each has one test of `%m`.
fn loops(text: &str) -> usize {
    text.lines().filter(|line| line.contains("icmp eq i16") && line.ends_with(", %m")).count()
}

const INPUTS: &[&[i128]] = &[&[0, 1], &[0, 5], &[1, 1], &[1, 6], &[7, 3]];

/// The loop `branch` is in: `%z` decides which of two blocks each trip runs.
fn branch(cond: &str) -> String {
    format!(
        "define i16 @f(i16 %n, i16 %m) {{
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %latch ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %latch ]
  {cond}
  br i1 %c, label %then, label %else

then:
  %a = add i16 %acc, %i
  br label %latch

else:
  %b = mul i16 %acc, 3
  br label %latch

latch:
  %sum = phi i16 [ %a, %then ], [ %b, %else ]
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %m
  br i1 %done, label %out, label %head

out:
  %r = phi i16 [ %sum, %latch ]
  ret i16 %r
}}
"
    )
}

/// `if (n == 0)` in the body ran on every trip: the loop was one loop that
/// asked again, where LLVM's makes a loop for each way and asks once.
#[test]
fn an_invariant_branch_in_the_body_makes_a_loop_for_each_way() {
    let text = branch("%c = and i1 %z, true");
    let after = unswitched(&text, INPUTS, true);
    assert_eq!(loops(&after), 2, "{after}");
    assert_eq!(loops(&unswitched(&text, INPUTS, false)), 1, "the trivial pass copied the loop");
}

/// The test is made once before the loops, on the invariant itself.
#[test]
fn the_unswitched_test_is_made_before_the_loops() {
    let text = branch("%c = and i1 %z, true").replace("br i1 %c, label %then", "br i1 %z, label %then");
    let after = unswitched(&text, INPUTS, true);
    assert_eq!(loops(&after), 2, "{after}");
    let entry = &after[after.find("b0:").expect("b0")..];
    let entry = entry.split("\n\n").next().expect("a block");
    assert!(entry.contains("br i1 %z"), "{after}");
}

/// Only the `and` side that is invariant is moved out; the loop that keeps
/// the branch still has the variant half.
#[test]
fn an_invariant_input_of_an_and_is_unswitched_in_the_one_direction_that_collapses_it() {
    let text = branch("%v = icmp slt i16 %i, 3\n  %c = and i1 %z, %v");
    let after = unswitched(&text, &[&[0, 5], &[1, 5], &[0, 1], &[4, 9]], true);
    assert_eq!(loops(&after), 2, "{after}");
    assert!(after.contains("and i1"), "the retained loop keeps the variant half: {after}");
}

#[test]
fn an_invariant_input_of_an_or_is_unswitched_in_the_one_direction_that_collapses_it() {
    let text = branch("%v = icmp slt i16 %i, 3\n  %c = or i1 %z, %v");
    let after = unswitched(&text, &[&[0, 5], &[1, 5], &[0, 1], &[4, 9]], true);
    assert_eq!(loops(&after), 2, "{after}");
}

/// A `select` on an invariant is made a branch first, then unswitched.
#[test]
fn a_select_on_an_invariant_is_unswitched() {
    let text = "define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %head ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %head ]
  %a = add i16 %acc, %i
  %b = mul i16 %acc, 3
  %sum = select i1 %z, i16 %a, i16 %b
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %m
  br i1 %done, label %out, label %head

out:
  %r = phi i16 [ %sum, %head ]
  ret i16 %r
}
";
    let after = unswitched(text, INPUTS, true);
    assert_eq!(loops(&after), 2, "{after}");
}

/// One loop for each target of the switch, the default included.
#[test]
fn a_switch_on_an_invariant_in_the_body_makes_a_loop_for_each_target() {
    let text = "define i16 @f(i16 %n, i16 %m) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %latch ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %latch ]
  switch i16 %n, label %d [
    i16 1, label %one
    i16 2, label %two
  ]

one:
  %a = add i16 %acc, %i
  br label %latch

two:
  %b = mul i16 %acc, 3
  br label %latch

d:
  %c = sub i16 %acc, %i
  br label %latch

latch:
  %sum = phi i16 [ %a, %one ], [ %b, %two ], [ %c, %d ]
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %m
  br i1 %done, label %out, label %head

out:
  %r = phi i16 [ %sum, %latch ]
  ret i16 %r
}
";
    let after = unswitched(text, &[&[1, 4], &[2, 4], &[3, 4], &[0, 1]], true);
    assert_eq!(loops(&after), 3, "{after}");
}

/// A value the loop makes and the code after reads is one phi after both
/// copies, whichever ran.
#[test]
fn a_value_read_after_the_loops_comes_from_whichever_copy_ran() {
    let text = branch("%c = and i1 %z, true");
    let after = unswitched(&text, INPUTS, true);
    assert!(after.contains("phi i16 [ %"), "{after}");
}

/// A loop too big to copy is left alone: LLVM's `unswitch-threshold` is 50.
#[test]
fn a_loop_whose_unswitched_cost_passes_the_threshold_is_left() {
    let filler: String = (0..60).map(|k| format!("%f{k} = add i16 %i, {k}\n  ")).collect();
    let text =
        branch("%c = and i1 %z, true").replace("%next = add i16 %i, 1", &format!("{filler}%next = add i16 %i, 1"));
    let after = unswitched(&text, INPUTS, true);
    assert_eq!(loops(&after), 1, "{after}");
}

/// The test on the inner loop's invariant is made once for the inner loop, in
/// the outer loop's body.
#[test]
fn an_inner_loop_is_unswitched_inside_the_outer_loop() {
    let text = "define i16 @f(i16 %n, i16 %m, i16 %k) {
b0:
  %z = icmp eq i16 %n, 0
  br label %outer

outer:
  %j = phi i16 [ 0, %b0 ], [ %jn, %olatch ]
  %acc = phi i16 [ 0, %b0 ], [ %r, %olatch ]
  br label %head

head:
  %i = phi i16 [ 0, %outer ], [ %next, %latch ]
  %x = phi i16 [ %acc, %outer ], [ %sum, %latch ]
  br i1 %z, label %then, label %else

then:
  %a = add i16 %x, %i
  br label %latch

else:
  %b = mul i16 %x, 3
  br label %latch

latch:
  %sum = phi i16 [ %a, %then ], [ %b, %else ]
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %m
  br i1 %done, label %olatch, label %head

olatch:
  %r = phi i16 [ %sum, %latch ]
  %jn = add i16 %j, 1
  %odone = icmp eq i16 %jn, %k
  br i1 %odone, label %out, label %outer

out:
  ret i16 %r
}
";
    let before = parsed(text);
    let mut module = before.clone();
    let after = managed(&mut module, SimpleLoopUnswitch { non_trivial: true });
    let inputs: &[&[i128]] = &[&[0, 3, 2], &[1, 3, 2], &[1, 1, 4]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    assert_eq!(loops(&after), 2, "{after}");
}
