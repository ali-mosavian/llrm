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

/// A struct filled by a memcpy and read by field: the copy becomes a load and
/// a store of each part (the stores promoted away), so the loads read the
/// source. With the memcpy escaping the alloca, it stays and so do the loads.
#[test]
fn test_a_struct_copied_by_memcpy_and_read_by_field_is_split() {
    let after = sroa(
        "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)

define i32 @f(ptr %src) {
b0:
  %s = alloca { i32, i32 }
  call void @llvm.memcpy.p0.p0.i16(ptr %s, ptr %src, i16 8, i1 false)
  %p1 = getelementptr i8, ptr %s, i16 4
  %x = load i32, ptr %s
  %y = load i32, ptr %p1
  %r = add i32 %x, %y
  ret i32 %r
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("call void @llvm.memcpy"), "{after}");
    assert_eq!(after.matches("load i32").count(), 2, "{after}");
}

/// A local set by a memset and read whole: the byte is spread over the integer
/// and the load is that constant.
#[test]
fn test_a_memset_of_a_local_then_a_load_is_the_constant() {
    let after = sroa(
        "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)

define i32 @f() {
b0:
  %s = alloca i32
  call void @llvm.memset.p0.i16(ptr %s, i8 1, i16 4, i1 false)
  %v = load i32, ptr %s
  ret i32 %v
}
",
    );
    assert!(
        !after.contains("alloca") && !after.contains("call void @llvm.memset") && !after.contains("load"),
        "{after}"
    );
    assert!(after.contains("ret i32 16843009"), "{after}");
}

/// Lifetime markers about a local do not keep it in memory.
#[test]
fn test_lifetime_markers_about_a_local_go_with_it() {
    let after = sroa(
        "declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)

define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  call void @llvm.lifetime.start.p0(i64 4, ptr %s)
  store i32 %a, ptr %s
  %v = load i32, ptr %s
  call void @llvm.lifetime.end.p0(i64 4, ptr %s)
  ret i32 %v
}
",
    );
    assert!(
        !after.contains("alloca") && !after.contains("call void @llvm.lifetime") && !after.contains("load"),
        "{after}"
    );
    assert!(after.contains("ret i32 %a"), "{after}");
}

/// A select of two locals' addresses, loaded: the load becomes a select of the
/// values (the loads speculated and promoted), so no memory is left. A select
/// that aborts the slice walk leaves both allocas.
#[test]
fn test_a_load_through_a_select_of_two_locals_is_speculated() {
    let after = sroa(
        "define i32 @f(i1 %c, i32 %a, i32 %b) {
b0:
  %x = alloca i32
  %y = alloca i32
  store i32 %a, ptr %x
  store i32 %b, ptr %y
  %p = select i1 %c, ptr %x, ptr %y
  %v = load i32, ptr %p
  ret i32 %v
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("load") && !after.contains("store"), "{after}");
    assert!(after.contains("select i1 %c, i32 %a, i32 %b"), "{after}");
}

/// A phi of two locals' addresses, loaded in its block: the loads go into the
/// predecessors and are promoted.
#[test]
fn test_a_load_through_a_phi_of_two_locals_is_speculated() {
    let after = sroa(
        "define i32 @f(i1 %c, i32 %a, i32 %b) {
b0:
  %x = alloca i32
  %y = alloca i32
  store i32 %a, ptr %x
  store i32 %b, ptr %y
  br i1 %c, label %t, label %e

t:
  br label %j

e:
  br label %j

j:
  %p = phi ptr [ %x, %t ], [ %y, %e ]
  %v = load i32, ptr %p
  ret i32 %v
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("load") && !after.contains("store"), "{after}");
    assert!(after.contains("phi i32 [ %a, %t ], [ %b, %e ]"), "{after}");
}
