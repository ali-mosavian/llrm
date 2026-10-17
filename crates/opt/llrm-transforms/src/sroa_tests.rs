//! Each test is a symptom: the text a rewrite must leave, which an alloca
//! left as it is does not.

use llrm_analysis::testing::DOS;
use llrm_mir::module::Module;

use super::Sroa;
use crate::testing::{managed, parsed};

fn module(body: &str) -> Module {
    parsed(&format!("{DOS}{body}"))
}

fn sroa(body: &str) -> String {
    managed(&mut module(body), Sroa)
}

/// A struct of two ints, each field stored and loaded by its own address: two
/// partitions, each promoted. The alloca, its stores and loads are gone.
#[test]
fn test_a_struct_of_two_ints_is_split_and_promoted() {
    let after = sroa(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %s = alloca { i32, i32 }
  %p0 = getelementptr { i32, i32 }, ptr %s, i32 0, i32 0
  %p1 = getelementptr { i32, i32 }, ptr %s, i32 0, i32 1
  store i32 %a, ptr %p0
  store i32 %b, ptr %p1
  %x = load i32, ptr %p0
  %y = load i32, ptr %p1
  %r = add i32 %x, %y
  ret i32 %r
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("add i32 %a, %b"), "{after}");
}

/// An array indexed by constant byte offsets: one partition per element.
#[test]
fn test_an_array_indexed_by_constants_is_split_and_promoted() {
    let after = sroa(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %s = alloca [2 x i32]
  %p1 = getelementptr i8, ptr %s, i16 4
  store i32 %a, ptr %s
  store i32 %b, ptr %p1
  %x = load i32, ptr %s
  %y = load i32, ptr %p1
  %r = sub i32 %x, %y
  ret i32 %r
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("sub i32 %a, %b"), "{after}");
}

/// An i64 written as two i32 halves and read whole: the load is split between
/// the partitions and the halves are put together with shifts and an `or`; no
/// memory is left.
#[test]
fn test_an_i64_written_as_two_i32_halves_is_widened_and_promoted() {
    let after = sroa(
        "define i64 @f(i32 %a, i32 %b) {
b0:
  %s = alloca i64
  %hi = getelementptr i8, ptr %s, i16 4
  store i32 %a, ptr %s
  store i32 %b, ptr %hi
  %v = load i64, ptr %s
  ret i64 %v
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("zext i32 %a") && after.contains("zext i32 %b") && after.contains(" or "), "{after}");
}

/// A first-class aggregate stored and loaded: split into its leaves first,
/// then promoted.
#[test]
fn test_an_aggregate_store_and_load_are_split_into_leaves_and_promoted() {
    let after = sroa(
        "define i32 @f({ i32, i32 } %v) {
b0:
  %s = alloca { i32, i32 }
  store { i32, i32 } %v, ptr %s
  %w = load { i32, i32 }, ptr %s
  %x = extractvalue { i32, i32 } %w, 1
  ret i32 %x
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load "), "{after}");
}

/// A pointer to the alloca goes to a call: it escapes, and nothing is split.
#[test]
fn test_an_escaping_alloca_stays() {
    let after = sroa(
        "declare void @use(ptr)

define i32 @f(i32 %a, i32 %b) {
b0:
  %s = alloca { i32, i32 }
  %p1 = getelementptr { i32, i32 }, ptr %s, i32 0, i32 1
  store i32 %a, ptr %s
  store i32 %b, ptr %p1
  call void @use(ptr %s)
  %x = load i32, ptr %p1
  ret i32 %x
}
",
    );
    assert!(after.contains("alloca { i32, i32 }") && after.contains("call void @use(ptr %s)"), "{after}");
    assert_eq!(after.matches("store").count(), 2, "{after}");
}

/// An array indexed by a variable: the offset is unknown, nothing is split.
#[test]
fn test_an_array_with_a_variable_index_stays() {
    let after = sroa(
        "define i32 @f(i32 %a, i16 %i) {
b0:
  %s = alloca [2 x i32]
  %p = getelementptr i32, ptr %s, i16 %i
  store i32 %a, ptr %s
  store i32 7, ptr %p
  %x = load i32, ptr %s
  ret i32 %x
}
",
    );
    assert!(after.contains("alloca [2 x i32]") && after.contains("store i32 7, ptr %p"), "{after}");
}

/// A volatile access cannot be promoted: the alloca and the access stay.
#[test]
fn test_a_volatile_store_keeps_its_alloca() {
    let after = sroa(
        "define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  store volatile i32 %a, ptr %s
  %x = load i32, ptr %s
  ret i32 %x
}
",
    );
    assert!(after.contains("alloca i32") && after.contains("store volatile i32 %a"), "{after}");
}
