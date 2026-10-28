//! `constraint-elimination` on MIR text: what the dominating conditions
//! decide becomes a constant, and nothing else changes.

use llrm_analysis::testing::m16_ir;
use llrm_mir::datalayout::DataLayout;

use crate::constraintelim::eliminate_constraints;
use crate::testing::{parsed, printed};

/// The text of the module after the pass has run on @f.
fn eliminated(text: &str) -> String {
    let mut module = parsed(text);
    let layout = DataLayout::default();
    let declarations = module.declarations();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    eliminate_constraints(context, &layout, &declarations, &metadata, function);
    printed(&module)
}

/// `x < 5` makes the inner `x < 10` true.
#[test]
fn a_dominating_compare_decides_an_implied_one() {
    let text = eliminated(
        "define i1 @f(i32 %x) {
b0:
  %c = icmp ult i32 %x, 5
  br i1 %c, label %b1, label %b2
b1:
  %d = icmp ult i32 %x, 10
  ret i1 %d
b2:
  ret i1 false
}
",
    );
    assert!(text.contains("ret i1 true"), "{text}");
}

/// A fact that does not decide the compare leaves it alone.
#[test]
fn an_undecided_compare_is_left_alone() {
    let text = eliminated(
        "define i1 @f(i32 %x) {
b0:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %b1, label %b2
b1:
  %d = icmp ult i32 %x, 5
  ret i1 %d
b2:
  ret i1 false
}
",
    );
    assert!(text.contains("ret i1 %d"), "{text}");
}

/// The fact of one arm does not hold in the other.
#[test]
fn a_fact_does_not_leave_its_arm() {
    let text = eliminated(
        "define i1 @f(i32 %x) {
b0:
  %c = icmp ult i32 %x, 5
  br i1 %c, label %b1, label %b2
b1:
  ret i1 false
b2:
  %d = icmp ult i32 %x, 10
  ret i1 %d
}
",
    );
    assert!(text.contains("ret i1 %d"), "{text}");
}

/// `for (i = 1919; i > -1; i--) check(i < 1920)`: the phi `{1919,+,-1}<nsw>`
/// bounds `i <= 1919` and the header test `i >= 0`, so the check is true.
/// Before the induction facts the check stayed (scroll.nib m16 ran 3.5x the
/// instructions: no `memmove` for the copy loop).
#[test]
fn a_decrementing_induction_decides_its_bounds_check() {
    let text = eliminated(
        "define void @f() {
entry:
  br label %h
h:
  %i = phi i16 [ 1919, %entry ], [ %n, %latch ]
  %c = icmp sgt i16 %i, -1
  br i1 %c, label %chk, label %x
chk:
  %ok = icmp ult i16 %i, 1920
  br i1 %ok, label %latch, label %x
latch:
  %n = add nsw i16 %i, -1
  br label %h
x:
  ret void
}
",
    );
    assert!(text.contains("br i1 true"), "{text}");
}

/// `nuw` follows the `nsw` the same check just added: `i + 1` with `0 <= i`
/// gets both. It got `nsw` alone while the second test read the flags from
/// before the first.
#[test]
fn nuw_is_judged_with_the_nsw_just_added() {
    let text = eliminated(
        "declare void @use(i16)
define void @f() {
entry:
  br label %h
h:
  %i = phi i16 [ 7, %entry ], [ %n, %b ]
  %c = icmp sge i16 %i, 0
  br i1 %c, label %b, label %x
b:
  %a = add i16 %i, 1
  call void @use(i16 %a)
  %n = add i16 %i, -1
  br label %h
x:
  ret void
}
",
    );
    assert!(text.contains("%a = add nuw nsw i16 %i, 1"), "{text}");
}

/// `v = g[i]` with a `nuw` index into a 10-element global: the access is in
/// bounds, so `i < 10`. The pass knew no object size and decided nothing.
#[test]
fn an_access_into_a_global_array_bounds_its_index() {
    let text = eliminated(
        "@g = global [10 x i32] zeroinitializer
define i1 @f(i64 %i) {
b0:
  %p = getelementptr inbounds nuw [10 x i32], ptr @g, i64 0, i64 %i
  %v = load i32, ptr %p
  %c = icmp ult i64 %i, 10
  ret i1 %c
}
",
    );
    assert!(text.contains("ret i1 true"), "{text}");
}

/// The same access after a call that may not return is not guaranteed to
/// execute: its fact joins the system after the access, in program order.
#[test]
fn an_access_that_may_not_run_bounds_what_follows_it() {
    let text = eliminated(
        "@g = global [10 x i32] zeroinitializer
declare void @maybe_exit()
define i1 @f(i64 %i) {
b0:
  call void @maybe_exit()
  %p = getelementptr inbounds nuw [10 x i32], ptr @g, i64 0, i64 %i
  %v = load i32, ptr %p
  %c = icmp ult i64 %i, 10
  ret i1 %c
}
",
    );
    assert!(text.contains("ret i1 true"), "{text}");
}

/// An index into an object of unknown size (an argument) gives nothing.
#[test]
fn an_access_into_an_unknown_object_gives_no_bound() {
    let text = eliminated(
        "define i1 @f(ptr %a, i64 %i) {
b0:
  %p = getelementptr inbounds nuw i32, ptr %a, i64 %i
  %v = load i32, ptr %p
  %c = icmp ult i64 %i, 10
  ret i1 %c
}
",
    );
    assert!(text.contains("ret i1 %c"), "{text}");
}

/// Every compare check worked out the position of every instruction of the
/// function again: 3.2 M of the 6.7 M Ir the pass took on a 1,283-instruction
/// d_faces body. The cost must not grow with the number of checks.
#[test]
fn the_positions_of_the_instructions_are_not_worked_out_per_check() {
    let positions_for = |checks: u32| {
        let mut body = String::from(
            "declare void @use(i1)\ndefine i1 @f(i32 %x) {\nb0:\n  %c0 = icmp ult i32 %x, 5\n  br i1 %c0, label %b1, label %b2\nb1:\n",
        );
        for n in 1..=checks {
            body.push_str(&format!("  %d{n} = icmp ult i32 %x, {}\n  call void @use(i1 %d{n})\n", 10 + n));
        }
        body.push_str(&format!("  ret i1 %d{checks}\nb2:\n  ret i1 false\n}}\n"));
        let before = llrm_mir::counts::get(llrm_mir::counts::Event::Positions);
        let text = eliminated(&body);
        assert!(text.contains("ret i1 true"), "{text}");
        llrm_mir::counts::get(llrm_mir::counts::Event::Positions) - before
    };
    assert_eq!(positions_for(1), positions_for(20));
}

/// `for (p = buf; p < buf + 100000; p++)` over a huge pointer was folded to
/// `br i1 false` and the loop deleted: the packed pointer was added as an
/// integer. The loop kept its guard.
#[test]
fn a_huge_pointer_loop_keeps_its_guard() {
    let text = m16_ir(
        "target datalayout = \"@m16-datalayout@-ni:2:3\"

define i32 @f(ptr addrspace(3) %buf) {
b0:
  br label %b1
b1:
  %s = phi i32 [ 0, %b0 ], [ %s2, %b2 ]
  %p = phi ptr addrspace(3) [ %buf, %b0 ], [ %p2, %b2 ]
  %end = getelementptr inbounds nuw i8, ptr addrspace(3) %buf, i32 100000
  %c = icmp ult ptr addrspace(3) %p, %end
  br i1 %c, label %b2, label %b3
b2:
  %v = load i8, ptr addrspace(3) %p
  %z = zext i8 %v to i32
  %s2 = add nsw i32 %s, %z
  %p2 = getelementptr inbounds nuw i8, ptr addrspace(3) %p, i32 1
  br label %b1
b3:
  ret i32 %s
}
",
    );
    let mut module = parsed(text);
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let declarations = module.declarations();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    eliminate_constraints(context, &layout, &declarations, &metadata, function);
    let text = printed(&module);
    assert!(text.contains("br i1 %c"), "{text}");
}
