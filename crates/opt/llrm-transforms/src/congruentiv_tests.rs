//! `replaceCongruentIVs`: counters SCEV proves equal are one counter. Shapes
//! and results as `opt -passes=indvars -disable-lftr -indvars-widen-indvars=0`
//! at the pin on `i486`.

use llrm_analysis::testing::layout;
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed};

/// @f of `text` with its congruent counters replaced, printed.
fn replaced(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("target datalayout = \"e-p:32:32-n8:16:32\"\n\n{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, Some(crate::indvars::tests::m32())));
    let (context, function) = module.function_mut("f").expect("@f");
    crate::loopsimplify::simplified(function);
    crate::lcssa::closed(function).expect("closes");
    let changed = super::replaced(context, &layout, function, &outer);
    (changed, printed(&module))
}

/// A `short` counter beside the `int` one, both from 0 by 1: the pin's `opt`
/// reads `trunc i32 %i to i16` where `%j` was.
#[test]
fn test_a_narrow_counter_that_is_a_truncation_of_a_wide_one_reads_it() {
    let (changed, after) = replaced(
        "define i32 @f(ptr %p, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %inc, %loop ]
  %j = phi i16 [ 0, %entry ], [ %jinc, %loop ]
  %a = getelementptr i32, ptr %p, i32 %i
  %jz = zext i16 %j to i32
  %b = getelementptr i32, ptr %p, i32 %jz
  store i32 %jz, ptr %a
  store i32 1, ptr %b
  %inc = add nuw nsw i32 %i, 1
  %jinc = add i16 %j, 1
  %c = icmp ne i32 %inc, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 0
}
",
    );
    assert!(changed, "{after}");
    assert!(after.contains("trunc i32 %i to i16"), "{after}");
    assert!(after.contains("zext i16 %indvars.iv1 to i32"), "{after}");
}

/// Two counters of one width, one start, one step: the second is the first.
#[test]
fn test_two_counters_with_one_start_and_step_are_one() {
    let (changed, after) = replaced(
        "define i32 @f(ptr %p, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %inc, %loop ]
  %k = phi i32 [ 0, %entry ], [ %kinc, %loop ]
  %a = getelementptr i32, ptr %p, i32 %i
  %b = getelementptr i32, ptr %p, i32 %k
  store i32 %k, ptr %a
  store i32 1, ptr %b
  %inc = add i32 %i, 1
  %kinc = add i32 %k, 1
  %c = icmp ne i32 %inc, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 0
}
",
    );
    assert!(changed, "{after}");
    assert!(after.contains("store i32 %i, ptr %a") && after.contains("ptr %a\n"), "{after}");
}

/// Counters from different starts are two.
#[test]
fn test_counters_from_different_starts_stay_two() {
    let (changed, after) = replaced(
        "define i32 @f(ptr %p, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %inc, %loop ]
  %k = phi i32 [ 1, %entry ], [ %kinc, %loop ]
  %a = getelementptr i32, ptr %p, i32 %i
  store i32 %k, ptr %a
  %inc = add i32 %i, 1
  %kinc = add i32 %k, 1
  %c = icmp ne i32 %inc, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 0
}
",
    );
    assert!(!changed, "{after}");
}
