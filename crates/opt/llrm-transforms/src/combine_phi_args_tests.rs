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

/// wasm32 r3 (an `i8` counter left over once the unroller used the trip
/// count): the counter's add fed only its own phi and survived to the
/// output, +20 B against the pin.
#[test]
fn an_induction_variable_nothing_reads_is_deleted() {
    let after = combined(
        "define i8 @f(i8 %n) {
b0:
  br label %loop
loop:
  %dead = phi i8 [ 0, %b0 ], [ %dead.next, %loop ]
  %live = phi i8 [ 0, %b0 ], [ %live.next, %loop ]
  %dead.next = add nuw i8 %dead, 4
  %live.next = add nuw i8 %live, 1
  %done = icmp eq i8 %live.next, %n
  br i1 %done, label %exit, label %loop
exit:
  ret i8 %live.next
}
",
    );
    assert!(!after.contains("%dead"), "{after}");
    assert!(after.contains("%live"), "{after}");
}

/// Phis that only read each other (a swap nothing observes) stayed, each one
/// a copy at lowering.
#[test]
fn phis_used_only_by_each_other_are_deleted() {
    let after = combined(
        "define i8 @f(i8 %n) {
b0:
  br label %loop
loop:
  %a = phi i8 [ 0, %b0 ], [ %b, %loop ]
  %b = phi i8 [ 1, %b0 ], [ %a, %loop ]
  %live = phi i8 [ 0, %b0 ], [ %live.next, %loop ]
  %live.next = add nuw i8 %live, 1
  %done = icmp eq i8 %live.next, %n
  br i1 %done, label %exit, label %loop
exit:
  ret i8 %live.next
}
",
    );
    assert_eq!(after.matches("phi i8").count(), 1, "{after}");
}

/// A web of 16 phis is left: the pin stops there to bound the walk.
#[test]
fn a_web_of_sixteen_phis_is_left() {
    let mut phis = String::new();
    for i in 0..16 {
        phis.push_str(&format!("  %p{i} = phi i8 [ 0, %b0 ], [ %p{}, %loop ]\n", (i + 1) % 16));
    }
    let after = combined(&format!(
        "define i8 @f(i8 %n) {{
b0:
  br label %loop
loop:
{phis}  %live = phi i8 [ 0, %b0 ], [ %live.next, %loop ]
  %live.next = add nuw i8 %live, 1
  %done = icmp eq i8 %live.next, %n
  br i1 %done, label %exit, label %loop
exit:
  ret i8 %live.next
}}
"
    ));
    assert!(after.contains("%p15 = phi"), "{after}");
}

/// stft_fixed~bounds m16 -O2: the bounds check's join read `phi(load a[i], load
/// a[0])`, a load in each arm where the pin's InstCombine has one load of a phi
/// of the addresses (its walk 321 -> 293 lines, ours 318).
#[test]
fn a_phi_of_loads_in_both_arms_is_one_load_of_a_phi_of_the_addresses() {
    let after = combined(
        "@a = global [8 x i32] zeroinitializer, align 4
define i32 @f(i1 %c, i16 %i) {
b0:
  br i1 %c, label %l, label %r
l:
  %p = getelementptr inbounds [4 x i8], ptr @a, i16 %i
  %x = load i32, ptr %p, align 4, !tbaa !0
  br label %m
r:
  %y = load i32, ptr @a, align 4, !tbaa !0
  br label %m
m:
  %v = phi i32 [ %x, %l ], [ %y, %r ]
  ret i32 %v
}
!0 = !{!1, !1, i64 0}
!1 = !{!\"int\", !2, i64 0}
!2 = !{!\"omnipotent char\"}
",
    );
    assert_eq!(after.matches("load i32").count(), 1, "{after}");
    assert!(after.contains("phi ptr") && !after.contains("phi i32"), "{after}");
    assert!(after.contains("!tbaa"), "the load keeps what both loads agree on: {after}");
}

/// The same address in every arm: no phi at all.
#[test]
fn a_phi_of_loads_of_one_address_is_one_load_of_it() {
    let after = combined(
        "@a = global i32 0, align 4
define i32 @f(i1 %c) {
b0:
  br i1 %c, label %l, label %r
l:
  %x = load i32, ptr @a, align 4
  br label %m
r:
  %y = load i32, ptr @a, align 4
  br label %m
m:
  %v = phi i32 [ %x, %l ], [ %y, %r ]
  ret i32 %v
}
",
    );
    assert_eq!(after.matches("load i32").count(), 1, "{after}");
    assert!(!after.contains("phi"), "{after}");
}

/// A write after the load, in its block, could change what the merged load
/// reads: the loads stay.
#[test]
fn a_store_after_a_load_keeps_the_loads_where_they_are() {
    let after = combined(
        "@a = global [8 x i32] zeroinitializer, align 4
@b = global i32 0, align 4
define i32 @f(i1 %c, i16 %i) {
b0:
  br i1 %c, label %l, label %r
l:
  %p = getelementptr inbounds [4 x i8], ptr @a, i16 %i
  %x = load i32, ptr %p, align 4
  store i32 1, ptr @b, align 4
  br label %m
r:
  %y = load i32, ptr @a, align 4
  br label %m
m:
  %v = phi i32 [ %x, %l ], [ %y, %r ]
  ret i32 %v
}
",
    );
    assert_eq!(after.matches("load i32").count(), 2, "{after}");
}

/// A volatile load is not moved.
#[test]
fn a_volatile_load_is_not_merged() {
    let after = combined(
        "@a = global [8 x i32] zeroinitializer, align 4
define i32 @f(i1 %c, i16 %i) {
b0:
  br i1 %c, label %l, label %r
l:
  %p = getelementptr inbounds [4 x i8], ptr @a, i16 %i
  %x = load volatile i32, ptr %p, align 4
  br label %m
r:
  %y = load volatile i32, ptr @a, align 4
  br label %m
m:
  %v = phi i32 [ %x, %l ], [ %y, %r ]
  ret i32 %v
}
",
    );
    assert_eq!(after.matches("load volatile i32").count(), 2, "{after}");
}

/// computed_goto: the dispatch sites each made `gep table, load` in their own
/// block. The pin's phi of them is one `gep` of a phi of the indices
/// (`foldPHIArgGEPIntoPHI`); ours kept both, 59 instructions after the
/// mid-end against the pin's 57.
#[test]
fn a_phi_of_geps_that_differ_in_one_index_is_a_gep_of_a_phi() {
    let after = combined(
        "define ptr @f(i1 %c, ptr %t, i32 %x, i32 %y) {
b0:
  br i1 %c, label %l, label %r
l:
  %a = getelementptr inbounds [4 x i8], ptr %t, i32 %x
  br label %m
r:
  %b = getelementptr inbounds [4 x i8], ptr %t, i32 %y
  br label %m
m:
  %p = phi ptr [ %a, %l ], [ %b, %r ]
  ret ptr %p
}
",
    );
    assert!(after.contains("phi i32"), "{after}");
    assert!(!after.contains("phi ptr"), "{after}");
    assert_eq!(after.matches("getelementptr").count(), 1, "{after}");
}

/// A constant index the other arm varies stays: the constant is the cheaper
/// arm, and a struct index must be one.
#[test]
fn a_phi_of_geps_with_a_constant_index_keeps_both() {
    let after = combined(
        "define ptr @f(i1 %c, ptr %t, i32 %y) {
b0:
  br i1 %c, label %l, label %r
l:
  %a = getelementptr inbounds [4 x i8], ptr %t, i32 2
  br label %m
r:
  %b = getelementptr inbounds [4 x i8], ptr %t, i32 %y
  br label %m
m:
  %p = phi ptr [ %a, %l ], [ %b, %r ]
  ret ptr %p
}
",
    );
    assert!(after.contains("phi ptr") && after.matches("getelementptr").count() == 2, "{after}");
}

/// Two operands that differ would need two phis: more registers where the
/// phis are, so the geps stay.
#[test]
fn a_phi_of_geps_that_differ_in_two_operands_keeps_both() {
    let after = combined(
        "define ptr @f(i1 %c, ptr %s, ptr %t, i32 %x, i32 %y) {
b0:
  br i1 %c, label %l, label %r
l:
  %a = getelementptr inbounds [4 x i8], ptr %s, i32 %x
  br label %m
r:
  %b = getelementptr inbounds [4 x i8], ptr %t, i32 %y
  br label %m
m:
  %p = phi ptr [ %a, %l ], [ %b, %r ]
  ret ptr %p
}
",
    );
    assert!(after.contains("phi ptr") && after.matches("getelementptr").count() == 2, "{after}");
}

/// A phi of one binary operator of two variables that share an operand made
/// the operation in both arms (stft_fixed's butterfly: the same `add` of two
/// variables in each arm, 286 instructions where the pin's InstCombine gives
/// 282): one operation of a phi of the other operands.
#[test]
fn a_phi_of_the_same_add_with_one_shared_operand_is_one_add_of_a_phi() {
    let after = combined(
        "define i32 @f(i1 %c, i32 %a, i32 %x, i32 %y) {
b0:
  br i1 %c, label %l, label %r
l:
  %u = add nsw i32 %a, %x
  br label %m
r:
  %v = add i32 %a, %y
  br label %m
m:
  %p = phi i32 [ %u, %l ], [ %v, %r ]
  ret i32 %p
}
",
    );
    assert_eq!(after.matches("add ").count(), 1, "{after}");
    assert!(after.contains("phi i32 [ %x, %l ], [ %y, %r ]"), "{after}");
    assert!(!after.contains("nsw"), "the flags of the arms intersect: {after}");
}

/// The shared operand may be on either side, and a compare folds too.
#[test]
fn a_phi_of_the_same_compare_with_one_shared_operand_is_one_compare_of_a_phi() {
    let after = combined(
        "define i1 @f(i1 %c, i32 %a, i32 %x, i32 %y) {
b0:
  br i1 %c, label %l, label %r
l:
  %u = icmp ult i32 %x, %a
  br label %m
r:
  %v = icmp ult i32 %y, %a
  br label %m
m:
  %p = phi i1 [ %u, %l ], [ %v, %r ]
  ret i1 %p
}
",
    );
    assert_eq!(after.matches("icmp ").count(), 1, "{after}");
}

/// Both operands differ: two phis for one operation would raise the pressure
/// in a loop header, and the pin leaves the phi.
#[test]
fn a_phi_of_adds_with_both_operands_different_is_left() {
    let after = combined(
        "define i32 @f(i1 %c, i32 %a, i32 %b, i32 %x, i32 %y) {
b0:
  br i1 %c, label %l, label %r
l:
  %u = add i32 %a, %x
  br label %m
r:
  %v = add i32 %b, %y
  br label %m
m:
  %p = phi i32 [ %u, %l ], [ %v, %r ]
  ret i32 %p
}
",
    );
    assert_eq!(after.matches("add ").count(), 2, "{after}");
}

/// A compare of another predicate is another operation.
#[test]
fn a_phi_of_compares_with_different_predicates_is_left() {
    let after = combined(
        "define i1 @f(i1 %c, i32 %a, i32 %x, i32 %y) {
b0:
  br i1 %c, label %l, label %r
l:
  %u = icmp ult i32 %a, %x
  br label %m
r:
  %v = icmp slt i32 %a, %y
  br label %m
m:
  %p = phi i1 [ %u, %l ], [ %v, %r ]
  ret i1 %p
}
",
    );
    assert_eq!(after.matches("icmp ").count(), 2, "{after}");
}
