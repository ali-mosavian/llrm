//! `CanonicalizeFreezeInLoops`: freezes of an induction variable go to the
//! preheader.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed, results};

/// `text`'s @f canonicalized, printed; the interpreter agrees on `inputs`.
fn canonicalized(
    text: &str,
    inputs: &[&[i128]],
) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = super::canonicalized(context, &layout, function, &outer);
    let after = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (changed, after)
}

fn freezes(text: &str) -> usize {
    text.matches("= freeze").count()
}

/// The header comment's example: the freeze of `i.next` is in the loop.
const EXAMPLE: &str = "define i16 @f(i16 %init, i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ %init, %entry ], [ %i.next, %loop ]
  %i.next = add nsw i16 %i, 1
  %i.next.fr = freeze i16 %i.next
  %c = icmp slt i16 %i.next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %i.next.fr
}
";

const INPUTS: &[&[i128]] = &[&[0, 5], &[3, 9], &[7, 2]];

/// Without the pass the freeze stayed in the loop and SCEV saw no recurrence.
#[test]
fn test_a_freeze_of_the_step_becomes_a_freeze_of_the_start_in_the_preheader() {
    let (changed, after) = canonicalized(EXAMPLE, INPUTS);
    assert!(changed, "{after}");
    assert_eq!(freezes(&after), 1, "{after}");
    let (preheader, rest) = after.split_once("loop:").expect("a loop block");
    assert!(preheader.contains("%init.frozen = freeze i16 %init"), "{after}");
    assert!(rest.contains("phi i16 [ %init.frozen, %entry ]"), "{after}");
    assert!(rest.contains("%i.next = add i16 %i, 1"), "nsw is dropped: {after}");
}

/// The step `%n + 1` is computed in the loop: freezing it would put a freeze
/// in the loop, so the phi is skipped. The same step computed before the loop
/// is frozen in the preheader.
#[test]
fn test_a_step_defined_in_the_loop_is_skipped_and_one_defined_outside_is_frozen() {
    let inside = "define i16 @f(i16 %init, i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ %init, %entry ], [ %i.next, %loop ]
  %s = add i16 %n, 1
  %i.next = add nsw i16 %i, %s
  %i.next.fr = freeze i16 %i.next
  %c = icmp slt i16 %i.next, 40
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %i.next.fr
}
";
    let (changed, after) = canonicalized(inside, INPUTS);
    assert!(!changed, "{after}");
    assert!(after.contains("%i.next.fr = freeze i16 %i.next"), "{after}");

    let outside = "define i16 @f(i16 %init, i16 %n) {
entry:
  %s = add i16 %n, 1
  br label %loop
loop:
  %i = phi i16 [ %init, %entry ], [ %i.next, %loop ]
  %i.next = add nsw i16 %i, %s
  %i.next.fr = freeze i16 %i.next
  %c = icmp slt i16 %i.next, 40
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %i.next.fr
}
";
    let (changed, after) = canonicalized(outside, INPUTS);
    assert!(changed, "{after}");
    let (preheader, rest) = after.split_once("loop:").expect("a loop block");
    assert!(preheader.contains("%s.frozen = freeze i16 %s"), "{after}");
    assert!(preheader.contains("%init.frozen = freeze i16 %init"), "{after}");
    assert!(!rest.contains("freeze"), "{after}");
}

/// A start and a step that cannot be poison need no freeze: the one in the
/// loop is only removed.
#[test]
fn test_values_that_are_not_poison_get_no_freeze() {
    let constants = EXAMPLE.replace("[ %init, %entry ]", "[ 0, %entry ]");
    let (changed, after) = canonicalized(&constants, INPUTS);
    assert!(changed, "{after}");
    assert_eq!(freezes(&after), 0, "{after}");
    assert!(after.contains("%i.next = add i16 %i, 1"), "{after}");
}

/// The loop's exit is also reached from outside it, so it is not in
/// LoopSimplify form and is left alone; with a dedicated exit it changes.
#[test]
fn test_a_loop_not_in_simplify_form_is_left_alone() {
    let shared = "define i16 @f(i16 %init, i16 %n) {
entry:
  %any = icmp sgt i16 %n, 0
  br i1 %any, label %pre, label %exit
pre:
  br label %loop
loop:
  %i = phi i16 [ %init, %pre ], [ %i.next, %loop ]
  %i.next = add nsw i16 %i, 1
  %i.next.fr = freeze i16 %i.next
  %c = icmp slt i16 %i.next, %n
  br i1 %c, label %loop, label %exit
exit:
  %r = phi i16 [ 0, %entry ], [ %i.next.fr, %loop ]
  ret i16 %r
}
";
    let (changed, after) = canonicalized(shared, INPUTS);
    assert!(!changed, "{after}");
    assert_eq!(freezes(&after), 1, "{after}");

    let dedicated = shared
        .replace("br i1 %c, label %loop, label %exit", "br i1 %c, label %loop, label %done")
        .replace(
            "exit:\n  %r = phi i16 [ 0, %entry ], [ %i.next.fr, %loop ]\n  ret i16 %r",
            "done:\n  br label %exit\nexit:\n  %r = phi i16 [ 0, %entry ], [ %i.next.fr, %done ]\n  ret i16 %r",
        );
    let (changed, after) = canonicalized(&dedicated, INPUTS);
    assert!(changed, "{after}");
    assert!(after.contains("%init.frozen = freeze i16 %init"), "{after}");
}
