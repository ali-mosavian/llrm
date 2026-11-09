//! Two functions are the same code when their bodies match; a differing constant, operation or shape separates them.

use super::{equivalent, function_hash};
use crate::testing::parsed;

const HEAD: &str = "target datalayout = \"e-p:32:32-i32:32-i64:32-n8:16:32\"\n\n";

fn same(text: &str, a: &str, b: &str) -> bool {
    let module = parsed(&format!("{HEAD}{text}"));
    let (ia, ib) = (module.named(a).unwrap(), module.named(b).unwrap());
    let (fa, fb) = (module.global(ia).function().unwrap(), module.global(ib).function().unwrap());
    let verdict = equivalent(&module, ia, fa, ib, fb);
    if verdict {
        assert_eq!(function_hash(fa), function_hash(fb), "equivalent functions share a hash");
    }
    verdict
}

const ADD: &str = "define i32 @a(i32 %x) {
b0:
  %r = add i32 %x, 1
  ret i32 %r
}

define i32 @b(i32 %y) {
entry:
  %s = add i32 %y, 1
  ret i32 %s
}

define i32 @c(i32 %y) {
b0:
  %s = add i32 %y, 2
  ret i32 %s
}

define i32 @d(i32 %y) {
b0:
  %s = sub i32 %y, 1
  ret i32 %s
}
";

/// gcc folds `__gtsf2` into `jmp __gesf2` and ours kept 16 B: two bodies that differ in names only are one.
#[test]
fn test_bodies_that_differ_in_names_only_are_equivalent() {
    assert!(same(ADD, "a", "b"));
}

/// A differing constant must not fold: it is another function.
#[test]
fn test_a_differing_constant_is_not_equivalent() {
    assert!(!same(ADD, "a", "c"));
}

#[test]
fn test_a_differing_operation_is_not_equivalent() {
    assert!(!same(ADD, "a", "d"));
}

/// Two directly recursive copies call themselves: the address of each is the other's.
#[test]
fn test_two_recursive_copies_are_equivalent() {
    let text = "define i32 @a(i32 %x) {
b0:
  %r = call i32 @a(i32 %x)
  ret i32 %r
}

define i32 @b(i32 %x) {
b0:
  %r = call i32 @b(i32 %x)
  ret i32 %r
}

define i32 @c(i32 %x) {
b0:
  %r = call i32 @a(i32 %x)
  ret i32 %r
}
";
    assert!(same(text, "a", "b"));
    assert!(!same(text, "b", "c"), "a call to another function is not the function's own address");
}

/// A loop's phi reads a value defined later in the walk: the pairing is checked where it is defined.
#[test]
fn test_loops_with_matching_phis_are_equivalent_and_swapped_ones_are_not() {
    let text = "define i32 @a(i32 %n) {
b0:
  br label %b1
b1:
  %i = phi i32 [ 0, %b0 ], [ %j, %b1 ]
  %j = add i32 %i, 1
  %c = icmp slt i32 %j, %n
  br i1 %c, label %b1, label %b2
b2:
  ret i32 %j
}

define i32 @b(i32 %m) {
b0:
  br label %b1
b1:
  %p = phi i32 [ 0, %b0 ], [ %q, %b1 ]
  %q = add i32 %p, 1
  %d = icmp slt i32 %q, %m
  br i1 %d, label %b1, label %b2
b2:
  ret i32 %q
}

define i32 @c(i32 %m) {
b0:
  br label %b1
b1:
  %p = phi i32 [ 0, %b0 ], [ %q, %b1 ]
  %q = add i32 %p, 1
  %d = icmp slt i32 %q, %m
  br i1 %d, label %b1, label %b2
b2:
  ret i32 %p
}
";
    assert!(same(text, "a", "b"));
    assert!(!same(text, "a", "c"), "returning the phi is not returning its successor");
}
