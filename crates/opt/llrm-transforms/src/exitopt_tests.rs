//! `optimizeLoopExits`: exits the counts decide get a constant condition.
//! The first two are `ult` and `ult_const_max` of LLVM's
//! `IndVarSimplify/eliminate-exit.ll`, in LoopSimplify form.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed};

/// `text`'s @f in LoopSimplify and LCSSA form with its exits optimized,
/// printed.
fn optimized(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::loopsimplify::simplified(function);
    crate::lcssa::closed(function).expect("closes");
    let changed = super::optimized(context, &layout, function, &mut declared, &outer);
    declared.place(&mut module).expect("places");
    (changed, printed(&module))
}

/// The second exit, `iv <u m`, cannot be taken before the first, `iv <u n`,
/// when the loop is entered only for `n <u m`.
#[test]
fn test_an_exit_an_earlier_exit_comes_before_is_never_taken() {
    let (changed, after) = optimized(
        "declare void @side_effect()
define void @f(i64 %n, i64 %m) {
entry:
  %cmp0 = icmp ult i64 %n, %m
  br i1 %cmp0, label %loop, label %exit
loop:
  %iv = phi i64 [ 0, %entry ], [ %iv.next, %latch ]
  %iv.next = add i64 %iv, 1
  %cmp1 = icmp ult i64 %iv, %n
  br i1 %cmp1, label %latch, label %exit
latch:
  call void @side_effect()
  %cmp2 = icmp ult i64 %iv, %m
  br i1 %cmp2, label %loop, label %exit
exit:
  ret void
}
",
    );
    assert!(changed, "{after}");
    assert!(after.contains("br i1 true, label %loop"), "{after}");
    assert!(after.contains("br i1 %cmp1, label %latch"), "{after}");
}

/// LLVM's `ult_const_max` with the first exit written `iv <u 20`: the loop's
/// maximum count, below `n`, is below the first exit's, so it is never taken.
/// (LLVM's own `udiv iv, 10 <u 2` needs `computeExitCountExhaustively`, which
/// is a stub here: D-T4.)
#[test]
fn test_an_exit_whose_count_exceeds_the_constant_maximum_is_never_taken() {
    let (changed, after) = optimized(
        "declare void @side_effect()
define void @f(i64 %n) {
entry:
  %cmp0 = icmp ult i64 %n, 20
  br i1 %cmp0, label %loop, label %exit
loop:
  %iv = phi i64 [ 0, %entry ], [ %iv.next, %latch ]
  %iv.next = add i64 %iv, 1
  %cmp1 = icmp ult i64 %iv, 20
  br i1 %cmp1, label %latch, label %exit
latch:
  call void @side_effect()
  %cmp2 = icmp ult i64 %iv, %n
  br i1 %cmp2, label %loop, label %exit
exit:
  ret void
}
",
    );
    assert!(changed, "{after}");
    assert!(after.contains("br i1 true, label %latch"), "{after}");
    assert!(after.contains("br i1 %cmp2, label %loop"), "{after}");
}

/// An exit that leaves on the first trip: the backedge is never taken, so the
/// header counter reads its preheader value.
#[test]
fn test_an_exit_taken_on_the_first_trip_replaces_the_header_phis_by_their_starts() {
    let (changed, after) = optimized(
        "define i16 @f(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %inc, %latch ]
  %s = phi i16 [ %n, %entry ], [ %s2, %latch ]
  %c = icmp ne i16 %i, 0
  br i1 %c, label %latch, label %exit
latch:
  %s2 = add i16 %s, %i
  %inc = add i16 %i, 1
  br label %loop
exit:
  ret i16 %s
}
",
    );
    assert!(changed, "{after}");
    assert!(after.contains("br i1 false, label %latch"), "{after}");
    assert!(after.contains("add i16 %n, 0") || after.contains("%s2 = add i16 %n"), "{after}");
}

/// Two exits at the same count: the later one is dead.
#[test]
fn test_a_second_exit_with_the_count_of_a_dominating_one_is_never_taken() {
    let (changed, after) = optimized(
        "define void @f(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %inc, %latch ]
  %c1 = icmp ne i16 %i, %n
  br i1 %c1, label %latch, label %exit
latch:
  %inc = add i16 %i, 1
  %c2 = icmp ne i16 %i, %n
  br i1 %c2, label %loop, label %exit
exit:
  ret void
}
",
    );
    assert!(changed, "{after}");
    assert!(after.contains("br i1 true, label %loop"), "{after}");
}

/// A leaf compare `iv <u m` of an exit whose count is unknown (the other leaf
/// reads memory), in a loop of at most 100 trips with `m >= 200`: true on every
/// trip, so the leaf is replaced by the constant. (Shape and result as
/// `opt -passes=indvars` at the pin.)
#[test]
fn test_a_leaf_compare_of_an_exit_with_no_count_that_holds_on_every_trip_is_true() {
    let (changed, after) = optimized(
        "define i32 @f(ptr %p, i32 %m) {
entry:
  %g = icmp uge i32 %m, 200
  br i1 %g, label %loop, label %exit
loop:
  %iv = phi i32 [ 0, %entry ], [ %iv.next, %body ]
  %c0 = icmp ult i32 %iv, 100
  br i1 %c0, label %body, label %exit
body:
  %a = getelementptr i32, ptr %p, i32 %iv
  %x = load i32, ptr %a
  %iv.next = add nuw i32 %iv, 1
  %c1 = icmp ult i32 %iv, %m
  %c2 = icmp ne i32 %x, 0
  %c = and i1 %c1, %c2
  br i1 %c, label %loop, label %exit
exit:
  ret i32 0
}
",
    );
    assert!(changed, "{after}");
    assert!(after.contains("%c = and i1 true, %c2"), "{after}");
}
