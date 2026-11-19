//! A strict exit compare of a unit-stride counter that never passes its bound.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed, results};

/// `text`'s @f with its latch compares strengthened, printed.
fn strengthened(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = super::strengthened(context, &layout, function, &outer);
    let after = printed(&module);
    let inputs: &[&[i128]] = &[&[0], &[1], &[2], &[7], &[100]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (changed, after)
}

/// A loop that sums `%i` while `test` holds, entered only when `n > 0`, `%i`
/// stepping by `step` from `n`; `%dec` is the stepped value.
fn counting_down(
    step: i32,
    test: &str,
    continues: &str,
) -> String {
    let (on_true, on_false) = if continues == "true" { ("loop", "exit") } else { ("exit", "loop") };
    format!(
        "define i32 @f(i32 %n) {{
entry:
  %g = icmp sgt i32 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i32 [ %n, %entry ], [ %dec, %loop ]
  %sum = phi i32 [ 0, %entry ], [ %acc, %loop ]
  %acc = add i32 %sum, %i
  %dec = add i32 %i, {step}
  %c = {test}
  br i1 %c, label %{on_true}, label %{on_false}
exit:
  %r = phi i32 [ 0, %entry ], [ %acc, %loop ]
  ret i32 %r
}}
"
    )
}

/// A loop from 0 up, `%inc` the stepped value, tested by `test`.
fn counting_up(test: &str) -> String {
    format!(
        "define i32 @f(i32 %n) {{
entry:
  %m = or i32 %n, 1
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %inc, %loop ]
  %inc = add i32 %i, 1
  %c = {test}
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %inc
}}
"
    )
}

#[test]
fn test_a_countdown_tested_above_its_floor_is_a_not_equal_test() {
    // memcpy's `while (n-- > 0)`: `n` positive on entry, `ugt %i, 1`. Left as
    // `ugt`, LSR cannot fold the counter into the pointer compare.
    let (changed, after) = strengthened(&counting_down(-1, "icmp ugt i32 %i, 1", "true"));
    assert!(changed, "{after}");
    assert!(after.contains("icmp ne i32 %i, 1"), "{after}");
}

#[test]
fn test_a_signed_countdown_is_a_not_equal_test() {
    let (changed, after) = strengthened(&counting_down(-1, "icmp sgt i32 %dec, 0", "true"));
    assert!(changed, "{after}");
    assert!(after.contains("icmp ne i32 %dec, 0"), "{after}");
}

#[test]
fn test_a_loop_left_on_true_gets_the_equal_test() {
    let (changed, after) = strengthened(&counting_down(-1, "icmp ule i32 %i, 1", "false"));
    assert!(changed, "{after}");
    assert!(after.contains("icmp eq i32 %i, 1"), "{after}");
}

#[test]
fn test_a_bound_on_the_left_is_read_swapped() {
    let (changed, after) = strengthened(&counting_down(-1, "icmp ult i32 1, %i", "true"));
    assert!(changed, "{after}");
    assert!(after.contains("icmp ne i32 1, %i"), "{after}");
}

#[test]
fn test_a_count_up_below_a_bound_it_starts_under_is_a_not_equal_test() {
    let (changed, after) = strengthened(&counting_up("icmp ult i32 %inc, 100"));
    assert!(changed, "{after}");
    assert!(after.contains("icmp ne i32 %inc, 100"), "{after}");
}

#[test]
fn test_a_count_up_below_a_bound_known_above_its_start_is_a_not_equal_test() {
    // `%m` is odd, so at least 1, the start of `%inc`.
    let (changed, after) = strengthened(&counting_up("icmp ult i32 %inc, %m"));
    assert!(changed, "{after}");
    assert!(after.contains("icmp ne i32 %inc, %m"), "{after}");
}

#[test]
fn test_a_count_up_below_a_bound_it_may_start_above_is_left() {
    // `%n` may be 0, under the start of `%inc`: the first test is then false
    // and `!=` would run on.
    let (changed, after) = strengthened(&counting_up("icmp ult i32 %inc, %n"));
    assert!(!changed, "{after}");
}

#[test]
fn test_a_countdown_with_no_entry_guard_is_left() {
    let text = counting_down(-1, "icmp ugt i32 %i, 1", "true").replace("icmp sgt i32 %n, 0", "icmp ne i32 %n, 1000");
    let (changed, after) = strengthened(&text);
    assert!(!changed, "{after}");
}

#[test]
fn test_a_countdown_guarded_only_above_zero_is_left_for_a_higher_floor() {
    // `n > 0` does not keep `%i` from starting under 5.
    let (changed, after) = strengthened(&counting_down(-1, "icmp ugt i32 %i, 5", "true"));
    assert!(!changed, "{after}");
}

#[test]
fn test_a_countdown_by_two_is_left() {
    // 7, 5, 3, 1, then wraps past the floor of 1 without meeting it.
    let (changed, after) = strengthened(&counting_down(-2, "icmp ugt i32 %i, 1", "true"));
    assert!(!changed, "{after}");
}

#[test]
fn test_a_countdown_tested_below_a_bound_is_left() {
    // The wrong side: the counter leaves the bound's side instead of
    // approaching it.
    let (changed, after) = strengthened(&counting_down(-1, "icmp ult i32 %i, 100", "true"));
    assert!(!changed, "{after}");
}

#[test]
fn test_a_test_in_the_header_of_a_loop_is_left() {
    // The test is not in the latch, so it does not decide each round.
    let (changed, after) = strengthened(
        "define i32 @f(i32 %n) {
entry:
  %g = icmp sgt i32 %n, 0
  br i1 %g, label %head, label %exit
head:
  %i = phi i32 [ %n, %entry ], [ %dec, %body ]
  %c = icmp ugt i32 %i, 1
  br i1 %c, label %body, label %exit
body:
  %dec = add i32 %i, -1
  br label %head
exit:
  ret i32 0
}
",
    );
    assert!(!changed, "{after}");
}
