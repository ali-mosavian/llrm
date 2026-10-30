//! Comparisons with an induction variable, eliminated by ScalarEvolution.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed, results};

fn simplified(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed =
        super::simplified_and_extended(context, &layout, function, &mut llrm_mir::passes::Declared::default(), &outer);
    (changed, printed(&module))
}

/// `n` is below 100, so `i < n` makes `i < 200` true on every trip: the
/// compare is `true` wherever it is read.
const BELOW: &str = "define i16 @f(i16 range(i16 0, 100) %n) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %next ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %body, label %done
body:
  %big = icmp ult i16 %i, 200
  br i1 %big, label %next, label %crash
next:
  %j = add nuw i16 %i, 1
  br label %head
crash:
  ret i16 99
done:
  ret i16 0
}
";

#[test]
fn test_a_compare_the_trip_count_decides_is_a_constant() {
    let (changed, after) = simplified(BELOW);
    assert!(changed, "{after}");
    assert!(after.contains("br i1 true, label %next"), "{after}");
    let before = parsed(&format!("{DOS}{BELOW}"));
    let mut module = parsed(&format!("{DOS}{BELOW}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    super::simplified_and_extended(context, &layout, function, &mut llrm_mir::passes::Declared::default(), &outer);
    assert_eq!(results(&module, &[&[0], &[5], &[99]]), results(&before, &[&[0], &[5], &[99]]));
}

/// A compare the trip count does not decide is left: `i < 50` for `n` below
/// 100 may fail.
#[test]
fn test_a_compare_nothing_decides_is_left() {
    // The flags `simplifyUsers` strengthens (`samesign`, `nsw`) are not a
    // decision.
    let (_, after) = simplified(&BELOW.replace("%i, 200", "%i, 50"));
    assert!(after.contains("br i1 %big"), "{after}");
}

/// QCport's `console` `pipeline.body` (-O1 and up, 8 modules and
/// `queens -O3`): a loop exit count asked while `getSCEVAtScope` was
/// computing an expression forgot that expression, and the scope's entry was
/// read back as held: `scope.rs` panicked `held`, so `simplifyindvar` took
/// the compile down.
const SCOPE_FORGETS_ITS_OWN_ENTRY: &str = r#"declare cc 1005 void @"_far_put@3"(ptr addrspace(1) nocapture writeonly, ptr nocapture readonly, i16) memory(argmem: readwrite) norecurse

define internal cc 1005 void @f(ptr addrspace(1) nocapture %v0, ptr nocapture readonly %v1) memory(read, argmem: readwrite, inaccessiblemem: none) norecurse {
b1:
  %v2 = alloca ptr addrspace(1)
  %v3 = alloca ptr
  %v4 = alloca i16
  %v5 = alloca [80 x i8]
  %v6 = alloca i16
  %v7 = alloca i16
  store ptr addrspace(1) %v0, ptr %v2
  store ptr %v1, ptr %v3
  %v8 = getelementptr inbounds i8, ptr addrspace(1) %v0, i16 12
  %v9 = getelementptr inbounds i8, ptr addrspace(1) %v0, i16 6
  %v10 = getelementptr inbounds i8, ptr addrspace(1) %v0, i16 1292
  %v11 = getelementptr inbounds i8, ptr addrspace(1) %v0, i16 8
  %v12 = getelementptr inbounds i8, ptr addrspace(1) %v0, i16 10
  br label %b2

b2:
  %v13 = phi ptr [ %v1, %b1 ]
  store i16 0, ptr %v6
  br label %b3

b3:
  %v14 = phi i16 [ 0, %b2 ], [ %v30, %b7 ]
  %v15 = getelementptr inbounds i8, ptr %v13, i16 %v14
  %v16 = load i8, ptr %v15
  %v17 = sext i8 %v16 to i16
  %v18 = icmp ne i8 %v16, 0
  %v19 = zext i1 %v18 to i16
  br i1 %v18, label %b5, label %b4

b4:
  store i16 %v14, ptr %v7
  %v20 = load i8, ptr %v15
  %v21 = sext i8 %v20 to i16
  %v22 = icmp ne i8 %v20, 0
  %v23 = zext i1 %v22 to i16
  br i1 %v22, label %b9, label %b8

b5:
  %v24 = load i8, ptr %v15
  %v25 = sext i8 %v24 to i16
  %v26 = icmp ne i16 %v25, 10
  %v27 = zext i1 %v26 to i16
  br i1 %v26, label %b6, label %b4

b6:
  %v28 = icmp slt i16 %v14, 38
  %v29 = zext i1 %v28 to i16
  br i1 %v28, label %b7, label %b4

b7:
  %v30 = add nsw i16 %v14, 1
  store i16 %v30, ptr %v6
  br label %b3

b8:
  %v31 = phi i16 [ %v14, %b4 ], [ %v14, %b9 ], [ %v14, %b10 ], [ %v38, %b13 ], [ %v14, %b17 ]
  ret void

b9:
  %v32 = load i8, ptr %v15
  %v33 = sext i8 %v32 to i16
  %v34 = icmp ne i16 %v33, 10
  %v35 = zext i1 %v34 to i16
  br i1 %v34, label %b10, label %b8

b10:
  %v36 = icmp eq i16 %v14, 38
  %v37 = zext i1 %v36 to i16
  br i1 %v36, label %b11, label %b8

b11:
  br label %b12

b12:
  %v38 = phi i16 [ %v14, %b11 ], [ %v48, %b15 ]
  %v39 = icmp sgt i16 %v38, 0
  %v40 = zext i1 %v39 to i16
  br i1 %v39, label %b14, label %b13

b13:
  %v41 = icmp eq i16 %v38, 0
  %v42 = zext i1 %v41 to i16
  br i1 %v41, label %b17, label %b8

b14:
  %v43 = getelementptr inbounds i8, ptr %v13, i16 %v38
  %v44 = load i8, ptr %v43
  %v45 = sext i8 %v44 to i16
  %v46 = icmp ne i16 %v45, 32
  %v47 = zext i1 %v46 to i16
  br i1 %v46, label %b15, label %b13

b15:
  %v48 = sub nsw i16 %v38, 1
  store i16 %v48, ptr %v7
  br label %b12

b17:
  store i16 %v14, ptr %v7
  br label %b8
}
"#;

#[test]
fn test_a_scope_that_forgets_its_own_expression_is_read_again() {
    let (_, after) = simplified(SCOPE_FORGETS_ITS_OWN_ENTRY);
    assert!(after.contains("define"), "{after}");
}

// ------------------------------------------------- simplifyUsers, beyond
// compares

/// @f of `text` through `indvars` with `simplifyAndExtend` on, printed, and
/// computing what it did on `inputs`.
fn extended(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    crate::testing::managed(&mut module, crate::indvars::IndVars { lftr: true, size: false });
    let after = printed(&module);
    assert_eq!(results(&module, inputs), before, "{after}");
    after
}

/// `for (i = 0; i < 100; i++) g = <body>` over i32; the body sees `%i`.
fn counted(body: &str) -> String {
    format!(
        "@g = global i32 0

define i32 @f(i32 %n) {{
b0:
  br label %h
h:
  %i = phi i32 [ 0, %b0 ], [ %in, %l ]
  %c = icmp slt i32 %i, 100
  br i1 %c, label %l, label %e
l:
{body}
  store i32 %v, ptr @g
  %in = add nsw i32 %i, 1
  br label %h
e:
  ret i32 0
}}
"
    )
}

#[test]
fn test_a_signed_division_of_a_nonnegative_counter_is_unsigned() {
    let after = extended(&counted("  %v = sdiv i32 %i, 3"), &[&[0], &[5]]);
    assert!(after.contains("udiv i32 %i, 3") && !after.contains("sdiv"), "{after}");
}

/// `i` is below 101 where the loop reads it (its range is `[0, 101)`): the
/// remainder by 101 is the counter.
#[test]
fn test_a_remainder_by_a_bound_the_counter_is_below_is_the_counter() {
    let after = extended(&counted("  %v = urem i32 %i, 101"), &[&[0], &[5]]);
    assert!(!after.contains("urem"), "{after}");
    assert!(after.contains("store i32 %i, ptr @g"), "{after}");
}

#[test]
fn test_a_signed_remainder_of_a_nonnegative_counter_is_unsigned() {
    let after = extended(&counted("  %v = srem i32 %i, 7"), &[&[3], &[7], &[200]]);
    assert!(after.contains("urem i32 %i, 7") && !after.contains("srem"), "{after}");
}

#[test]
fn test_an_add_of_zero_to_the_counter_is_the_counter() {
    let after = extended(&counted("  %v = add i32 %i, 0"), &[&[0], &[5]]);
    assert!(!after.contains("add i32 %i, 0") && after.contains("store i32 %i, ptr @g"), "{after}");
}

#[test]
fn test_an_increment_that_cannot_wrap_gains_its_flags() {
    let after = extended(&counted("  %v = add i32 %i, 1"), &[&[0], &[5]]);
    assert!(after.contains("add nuw nsw i32"), "{after}");
}

/// `((i + 1) >> 2)` of a counter that steps by 4 from 0 is `i >> 2`: the
/// increment is folded into the use.
#[test]
fn test_an_increment_an_aligned_counter_makes_no_difference_to_is_folded_into_its_use() {
    let text = "@g = global i32 0

define i32 @f(i32 %n) {
b0:
  br label %h
h:
  %i = phi i32 [ 0, %b0 ], [ %in, %l ]
  %c = icmp ult i32 %i, 400
  br i1 %c, label %l, label %e
l:
  %j = add nuw i32 %i, 1
  %v = lshr i32 %j, 2
  store i32 %v, ptr @g
  %in = add nuw nsw i32 %i, 4
  br label %h
e:
  ret i32 0
}
";
    let after = extended(text, &[&[0]]);
    assert!(after.contains("lshr i32 %i, 2"), "{after}");
    assert!(!after.contains("lshr i32 %j"), "{after}");
}

/// `icmp ult (trunc i), n16` where `i` is below 100: the compare is on the
/// counter against the extended bound, and the `trunc` goes.
#[test]
fn test_a_compare_of_a_truncated_counter_is_a_compare_of_the_counter() {
    let text = "@g = global i32 0

define i32 @f(i16 %n) {
b0:
  br label %h
h:
  %i = phi i32 [ 0, %b0 ], [ %in, %l ]
  %c = icmp slt i32 %i, 100
  br i1 %c, label %l, label %e
l:
  %t = trunc i32 %i to i16
  %k = icmp ult i16 %t, %n
  %v = zext i1 %k to i32
  store i32 %v, ptr @g
  %in = add nsw i32 %i, 1
  br label %h
e:
  ret i32 0
}
";
    let after = extended(text, &[&[0], &[50], &[200]]);
    assert!(!after.contains("trunc"), "{after}");
}

/// The inner loop starts `j` at the outer counter `o` and leaves when
/// `j >= o`: the first trip decides it, so the compare reads the same values
/// every trip and goes to the preheader as `o >= o`, true. The compare of
/// `bench/shellsort`'s insertion loop stayed in the loop, with a block that
/// never ran, because `makeIVComparisonInvariant` was not translated.
#[test]
fn test_a_compare_of_a_counter_with_its_start_is_decided_by_the_first_trip() {
    let text = "define i16 @f(i16 %n) {
entry:
  br label %outer
outer:
  %o = phi i16 [ 32, %entry ], [ %on, %latch ]
  %more = icmp slt i16 %o, %n
  br i1 %more, label %pre, label %done
pre:
  br label %inner
inner:
  %j = phi i16 [ %o, %pre ], [ %jn, %step ]
  %big = icmp sge i16 %j, %o
  br i1 %big, label %latch, label %step
step:
  %jn = add nuw nsw i16 %j, 1
  br label %inner
latch:
  %on = add nuw nsw i16 %o, 1
  br label %outer
done:
  ret i16 0
}
";
    let (_, after) = simplified(text);
    assert!(after.contains("br i1 true, label %latch"), "{after}");
}
