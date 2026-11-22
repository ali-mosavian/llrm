//! `predicateLoopExits`: a read-only loop's exits test the trip count before
//! the loop. The shape and result are `opt -passes=indvars -disable-lftr` at
//! the pin on the same body.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed};

/// `text`'s @f in LoopSimplify and LCSSA form with its exits predicated,
/// printed.
fn predicated(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::loopsimplify::simplified(function);
    crate::lcssa::closed(context, &crate::testing::layout(), function).expect("closes");
    let changed = super::predicated(context, &layout, function, &mut declared, &outer);
    declared.place(&mut module).expect("places");
    (changed, printed(&module))
}

const TWO_EXITS: &str = "define i32 @f(i32 %n, i32 %m) {
entry:
  br label %loop
loop:
  %iv = phi i32 [ 0, %entry ], [ %iv.next, %latch ]
  %c1 = icmp ne i32 %iv, %n
  br i1 %c1, label %latch, label %exit1
latch:
  %iv.next = add nuw i32 %iv, 1
  %c2 = icmp ne i32 %iv.next, %m
  br i1 %c2, label %loop, label %exit2
exit1:
  ret i32 1
exit2:
  ret i32 2
}
";

/// Two exits at counts `n` and `m - 1`: each tests its count against the
/// loop's, before the loop, and the counter no longer decides either.
#[test]
fn test_each_exit_of_a_read_only_loop_tests_its_count_before_the_loop() {
    let (changed, after) = predicated(TWO_EXITS);
    assert!(changed, "{after}");
    assert!(!after.contains("br i1 %c1,") && !after.contains("br i1 %c2,"), "{after}");
    let entry = after.split("loop:").next().expect("a preheader");
    assert!(entry.matches("icmp ne i32").count() >= 2, "{entry}");
}

/// A store to memory the exit can see keeps the loop as it was.
#[test]
fn test_a_loop_that_writes_memory_is_not_predicated() {
    let (changed, after) = predicated(
        &TWO_EXITS
            .replace("define i32 @f(i32 %n, i32 %m) {", "define i32 @f(i32 %n, i32 %m, ptr %p) {")
            .replace("  %c1 =", "  store i32 %iv, ptr %p\n  %c1 ="),
    );
    assert!(!changed, "{after}");
}

/// Without LCSSA a loop's only exit, read outside through the loop's own value,
/// was made `br i1 false`: the loop left on its first trip (a 256-trip byte
/// loop summed one element). The exit stays.
#[test]
fn test_a_live_out_read_without_an_exit_phi_keeps_the_exit() {
    let mut module = parsed(&format!(
        "{DOS}define i32 @f(i32 %n) {{
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i.next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %t, %loop ]
  %t = add i32 %s, %i
  %i.next = add nuw i32 %i, 1
  %c = icmp ne i32 %i.next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %t
}}
"
    ));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::loopsimplify::simplified(function);
    let changed = super::predicated(context, &layout, function, &mut declared, &outer);
    declared.place(&mut module).expect("places");
    let after = printed(&module);
    assert!(!changed && after.contains("br i1 %c,"), "{after}");
}
