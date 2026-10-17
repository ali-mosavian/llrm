//! Tests of the EarlyCSE translation (`early-cse<memssa>`): what its scoped
//! tables and generation count do.

use llrm_analysis::testing::DOS;

use super::EarlyCse;
use crate::testing::{managed, parsed};

fn run(body: &str) -> String {
    managed(&mut parsed(&format!("{DOS}{body}")), EarlyCse)
}

/// A commuted add was a second add, which `gvn` found much later.
#[test]
fn test_a_commuted_operation_is_the_one_before() {
    let after = run("define i32 @f(i32 %x, i32 %y) {
b0:
  %a = add i32 %x, %y
  %b = add i32 %y, %x
  %c = mul i32 %a, %b
  ret i32 %c
}
");
    assert_eq!(after.matches("add i32").count(), 1, "{after}");
}

/// A compare with its operands exchanged and the predicate swapped is the same
/// value.
#[test]
fn test_a_swapped_compare_is_the_one_before() {
    let after = run("define i1 @f(i32 %x, i32 %y) {
b0:
  %a = icmp slt i32 %x, %y
  %b = icmp sgt i32 %y, %x
  %c = and i1 %a, %b
  ret i1 %c
}
");
    assert_eq!(after.matches("icmp").count(), 1, "{after}");
}

/// A select on the inverted condition with its arms exchanged is the same
/// value.
#[test]
fn test_an_inverted_select_is_the_one_before() {
    let after = run("define i32 @f(i32 %x, i32 %y, i32 %p, i32 %q) {
b0:
  %c1 = icmp slt i32 %x, %y
  %c2 = icmp sge i32 %x, %y
  %a = select i1 %c1, i32 %p, i32 %q
  %b = select i1 %c2, i32 %q, i32 %p
  %r = add i32 %a, %b
  ret i32 %r
}
");
    assert_eq!(after.matches("select").count(), 1, "{after}");
}

/// A load of a cell just stored is the stored value.
#[test]
fn test_a_load_after_a_store_is_the_stored_value() {
    let after = run("define i32 @f(ptr %p, i32 %v) {
b0:
  store i32 %v, ptr %p
  %l = load i32, ptr %p
  ret i32 %l
}
");
    assert!(!after.contains("load") && after.contains("ret i32 %v"), "{after}");
}

/// Two loads with no write between are one.
#[test]
fn test_a_second_load_is_the_first() {
    let after = run("define i32 @f(ptr %p) {
b0:
  %a = load i32, ptr %p
  %b = load i32, ptr %p
  %r = add i32 %a, %b
  ret i32 %r
}
");
    assert_eq!(after.matches("load").count(), 1, "{after}");
}

/// Of two stores to a cell with nothing reading it between, the first is dead.
#[test]
fn test_a_store_overridden_by_the_next_is_removed() {
    let after = run("define void @f(ptr %p) {
b0:
  store i32 1, ptr %p
  store i32 2, ptr %p
  ret void
}
");
    assert_eq!(after.matches("store").count(), 1, "{after}");
    assert!(after.contains("store i32 2"), "{after}");
}

/// A call that may write the cell ends what was loaded from it.
#[test]
fn test_a_load_is_not_forwarded_across_a_call_that_may_write() {
    let after = run("declare void @opaque(ptr)

define i32 @f(ptr %p) {
b0:
  %a = load i32, ptr %p
  call void @opaque(ptr %p)
  %b = load i32, ptr %p
  %r = add i32 %a, %b
  ret i32 %r
}
");
    assert_eq!(after.matches("load").count(), 2, "{after}");
}

/// The branch taken tells the condition: in the arm a repeat of it is a
/// constant.
#[test]
fn test_a_condition_is_known_in_the_arm_it_chose() {
    let after = run("define i1 @f(i32 %x) {
b0:
  %c = icmp eq i32 %x, 0
  br i1 %c, label %t, label %e

t:
  %d = icmp eq i32 %x, 0
  ret i1 %d

e:
  ret i1 false
}
");
    assert!(after.contains("ret i1 true"), "{after}");
}
