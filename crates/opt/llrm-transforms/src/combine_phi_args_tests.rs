//! `combine_phi_args`: a phi of one operation is the operation of a phi.

use llrm_mir::datalayout::DataLayout;

use crate::combine::{combine_with, visit};
use crate::testing::{parsed, printed};

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::parse("e-n8:16:32").expect("parses"), function, visit);
    printed(&module)
}

/// clip_w: `phi i32` of `zext i1` kept an `icmp eq i32 %p, 0` per use where
/// the pin's phi is of the `i1`s: inline cost 235 against the pin's 220.
#[test]
fn a_phi_of_zexts_of_booleans_is_a_zext_of_a_boolean_phi() {
    let after = combined(
        "define i1 @f(i1 %c, i1 %x, i1 %y) {
b0:
  br i1 %c, label %l, label %r
l:
  %a = zext i1 %x to i32
  br label %m
r:
  %b = zext i1 %y to i32
  br label %m
m:
  %p = phi i32 [ %a, %l ], [ %b, %r ]
  %e = icmp eq i32 %p, 0
  ret i1 %e
}
",
    );
    assert!(after.contains("phi i1"), "{after}");
    assert!(!after.contains("phi i32") && !after.contains("icmp eq i32"), "{after}");
}

/// Three or more arms, zexts of one type and a constant that fits it: a
/// narrow phi.
#[test]
fn zexts_and_a_fitting_constant_make_a_narrow_phi() {
    let after = combined(
        "define i32 @f(i2 %s, i8 %x, i8 %y) {
b0:
  switch i2 %s, label %d [ i2 0, label %l i2 1, label %r ]
l:
  %a = zext i8 %x to i32
  br label %m
r:
  %b = zext i8 %y to i32
  br label %m
d:
  br label %m
m:
  %p = phi i32 [ %a, %l ], [ %b, %r ], [ 7, %d ]
  ret i32 %p
}
",
    );
    assert!(after.contains("phi i8 [ %x, %l ], [ %y, %r ], [ 7, %d ]"), "{after}");
    assert!(after.contains("zext i8 %"), "{after}");
}

/// A constant past the narrow type, or a zext with a second user, stays.
#[test]
fn an_unfitting_constant_or_a_shared_zext_keeps_the_wide_phi() {
    let wide = combined(
        "define i32 @f(i2 %s, i8 %x, i8 %y) {
b0:
  switch i2 %s, label %d [ i2 0, label %l i2 1, label %r ]
l:
  %a = zext i8 %x to i32
  br label %m
r:
  %b = zext i8 %y to i32
  br label %m
d:
  br label %m
m:
  %p = phi i32 [ %a, %l ], [ %b, %r ], [ 300, %d ]
  ret i32 %p
}
",
    );
    assert!(wide.contains("phi i32"), "{wide}");
    let shared = combined(
        "define i32 @f(i1 %c, i8 %x, i8 %y, ptr %g) {
b0:
  br i1 %c, label %l, label %r
l:
  %a = zext i8 %x to i32
  store i32 %a, ptr %g
  br label %m
r:
  %b = zext i8 %y to i32
  br label %m
m:
  %p = phi i32 [ %a, %l ], [ %b, %r ]
  ret i32 %p
}
",
    );
    assert!(shared.contains("phi i32"), "{shared}");
}

/// The same binary operator with one constant right operand moves past the
/// phi.
#[test]
fn a_phi_of_adds_of_one_constant_is_an_add_of_a_phi() {
    let after = combined(
        "define i32 @f(i1 %c, i32 %x, i32 %y) {
b0:
  br i1 %c, label %l, label %r
l:
  %a = add i32 %x, 5
  br label %m
r:
  %b = add i32 %y, 5
  br label %m
m:
  %p = phi i32 [ %a, %l ], [ %b, %r ]
  ret i32 %p
}
",
    );
    assert!(after.contains("phi i32 [ %x, %l ], [ %y, %r ]") && after.contains("add i32 %"), "{after}");
    assert!(after.matches("add i32").count() == 1, "{after}");
}

/// mandel -O1: `phi i1 [ false, %b0 ], [ true, %a ]` under `br i1 %c, %j, %a`
/// stayed a phi, materialised in a register and tested again; the pin reads
/// `xor %c, true`.
#[test]
fn a_phi_of_booleans_under_a_branch_is_the_negated_condition() {
    let after = combined(
        "define i1 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %j, label %a
a:
  call void @g(i16 %x)
  br label %j
j:
  %p = phi i1 [ false, %b0 ], [ true, %a ]
  ret i1 %p
}
declare void @g(i16)
",
    );
    assert!(!after.contains("phi"), "{after}");
    assert!(after.contains("xor i1 %c, true"), "{after}");
}

/// The same phi with the arms the branch's own way is the condition.
#[test]
fn a_phi_of_booleans_the_branch_orders_is_the_condition() {
    let after = combined(
        "define i1 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %j, label %a
a:
  call void @g(i16 %x)
  br label %j
j:
  %p = phi i1 [ true, %b0 ], [ false, %a ]
  ret i1 %p
}
declare void @g(i16)
",
    );
    assert!(!after.contains("phi") && after.contains("ret i1 %c"), "{after}");
}

/// `simplifyUsingControlFlow`'s switch form: a phi of the case values, each
/// from its own case's block, is the switch's condition.
#[test]
fn a_phi_of_case_values_is_the_switch_condition() {
    let after = combined(
        "define i8 @f(i8 %s) {
b0:
  switch i8 %s, label %d [ i8 1, label %a i8 2, label %b ]
a:
  br label %j
b:
  br label %j
d:
  ret i8 0
j:
  %p = phi i8 [ 1, %a ], [ 2, %b ]
  ret i8 %p
}
",
    );
    assert!(!after.contains("phi") && after.contains("ret i8 %s"), "{after}");
}
