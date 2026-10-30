//! `canonicalizeExitCondition`: an exit compare of a zero-extended counter.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed, results};

/// `text`'s @f with its exit conditions canonicalized, printed.
fn canonicalized(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    crate::lcssa::closed(function).expect("closes");
    let changed = super::canonicalized(context, &layout, function, &outer);
    let after = printed(&module);
    let inputs: &[&[i128]] = &[&[0], &[1], &[7], &[100]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (changed, after)
}

/// An `i16` counter widened to `i32` for a signed test against `%m`, which is
/// at most 255 (`and`), so it fits `i16`.
fn widened(test: &str) -> String {
    format!(
        "define i16 @f(i32 %n) {{
entry:
  %m = and i32 %n, 255
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %inc, %loop ]
  %inc = add nuw i16 %i, 1
  %w = zext i16 %inc to i32
  %c = {test}
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %inc
}}
"
    )
}

#[test]
fn test_a_signed_test_of_an_extended_counter_against_a_small_bound_is_unsigned_then_narrow() {
    let (changed, after) = canonicalized(&widened("icmp slt i32 %w, %m"));
    assert!(changed, "{after}");
    assert!(after.contains("icmp ult i16 %inc,"), "{after}");
    assert!(after.contains("trunc i32 %m to i16"), "{after}");
}

#[test]
fn test_a_bound_that_may_not_fit_the_counter_leaves_the_test() {
    let (changed, after) = canonicalized(&widened("icmp slt i32 %w, %n"));
    assert!(!changed, "{after}");
}
