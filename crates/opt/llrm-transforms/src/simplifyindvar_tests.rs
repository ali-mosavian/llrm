//! Comparisons with an induction variable, eliminated by ScalarEvolution.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed, results};

thread_local! {
    /// Forces `widening_enabled` in this test's thread.
    pub static FORCED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// `simplifyAndExtend` is opt-in (`LLRM_SCEV_WIDEN`) until #2531 is fixed: a
/// run without the variable takes the old path. Its panic at `widen.rs:142`
/// (a user with no value) stopped every smoke program at -O2 -m32.
#[test]
fn test_widening_by_scalar_evolution_is_off_unless_asked_for() {
    assert!(!super::widening_enabled());
}

fn simplified(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = super::simplified(context, &layout, function, &outer);
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
    super::simplified(context, &layout, function, &outer);
    assert_eq!(results(&module, &[&[0], &[5], &[99]]), results(&before, &[&[0], &[5], &[99]]));
}

/// A compare the trip count does not decide is left: `i < 50` for `n` below
/// 100 may fail.
#[test]
fn test_a_compare_nothing_decides_is_left() {
    let (changed, after) = simplified(&BELOW.replace("%i, 200", "%i, 50"));
    assert!(!changed, "{after}");
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
