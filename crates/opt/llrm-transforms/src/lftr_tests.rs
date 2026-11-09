//! `linearFunctionTestReplace`: the exit test as `counter != limit`.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed, results};

/// `text`'s @f in LCSSA form with its exit tests replaced, printed.
fn replaced(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::lcssa::closed(context, &crate::testing::layout(), function).expect("closes");
    let changed = super::replaced(context, &layout, function, &mut declared, &outer);
    let after = printed(&module);
    let inputs: &[&[i128]] = &[&[0], &[1], &[7], &[100]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (changed, after)
}

/// Ten trips of `s += 2`, the test an unsigned bound on the increment.
fn counted(
    test: &str,
    step: u32,
) -> String {
    format!(
        "define i16 @f(i16 %n) {{
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %inc, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s2, %loop ]
  %s2 = add i16 %s, %n
  %inc = add i16 %i, {step}
  %c = {test}
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %s2
}}
"
    )
}

#[test]
fn test_a_less_than_test_on_a_counter_becomes_not_equal_to_the_limit() {
    let (changed, after) = replaced(&counted("icmp ult i16 %inc, 10", 1));
    assert!(changed, "{after}");
    assert!(after.contains("%exitcond = icmp ne i16 %inc, 10"), "{after}");
}

#[test]
fn test_a_test_already_not_equal_on_a_counter_is_left() {
    let (changed, after) = replaced(&counted("icmp ne i16 %inc, 10", 1));
    assert!(!changed, "{after}");
}

/// The pin's `isLoopCounter` wants a step of one (`m_scev_One`): a counter
/// that steps by two keeps its `ult` test even where it cannot wrap. The
/// removed deviation LFTR1 made it `icmp ne` (wasm `scroll_down` +29 B for the
/// same rewrite of a `--i` loop).
#[test]
fn test_a_stride_of_two_is_not_a_counter() {
    for bound in [10, 9] {
        let (changed, after) =
            replaced(&counted(&format!("icmp ult i16 %inc, {bound}"), 2).replace("add i16 %i,", "add nuw i16 %i,"));
        assert!(!changed, "bound {bound}: {after}");
        assert!(!after.contains("%exitcond"), "bound {bound}: {after}");
    }
}

/// Exiting from the header, not the latch, the test reads the counter before
/// its increment.
#[test]
fn test_a_test_in_the_header_compares_the_counter_before_its_increment() {
    let (changed, after) = replaced(
        "define i16 @f(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %inc, %body ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %body, label %exit
body:
  %inc = add nsw i16 %i, 1
  br label %loop
exit:
  ret i16 %i
}
",
    );
    assert!(changed, "{after}");
    assert!(after.contains("%exitcond = icmp ne i16 %i, 10"), "{after}");
}

/// Strides of 3, 4 and 8 are not counters either.
#[test]
fn test_larger_strides_are_not_counters() {
    for (step, bound) in [(3, 10), (4, 10), (8, 100)] {
        let text = counted(&format!("icmp ult i16 %inc, {bound}"), step).replace("add i16 %i,", "add nuw i16 %i,");
        let (changed, after) = replaced(&text);
        assert!(!changed, "step {step}: {after}");
    }
}

/// A counter that steps down is not one: `for (i = n - 1; i >= 0; --i)` keeps
/// its signed compare (wasm idioms `scroll_down`).
#[test]
fn test_a_negative_stride_is_not_a_counter() {
    let (changed, after) = replaced(
        "define i16 @f(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 20, %entry ], [ %inc, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s2, %loop ]
  %s2 = add i16 %s, %n
  %inc = add nsw i16 %i, -2
  %c = icmp sgt i16 %inc, 0
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %s2
}
",
    );
    assert!(!changed, "{after}");
    assert!(!after.contains("%exitcond"), "{after}");
}

/// shellsort's inner loop (`at -= gap` while `at >= gap`) trips `at / gap`
/// times: its limit takes a division. A stride of two over a bound only known
/// at run time is left alone (no counter, so no limit to expand).
#[test]
fn test_a_stride_whose_limit_needs_code_is_left_alone() {
    let (changed, after) = replaced(&counted("icmp ult i16 %inc, %n", 2).replace("add i16 %i,", "add nuw i16 %i,"));
    assert!(!changed, "{after}");
}
