//! Each test is a symptom: the text a promotion must leave, which an
//! unpromoted alloca does not.

use llrm_analysis::testing::DOS;
use llrm_mir::module::Module;

use super::Mem2Reg;
use crate::testing::{managed, parsed};

fn module(body: &str) -> Module {
    parsed(&format!("{DOS}{body}"))
}

fn promoted(body: &str) -> String {
    managed(&mut module(body), Mem2Reg)
}

/// One store that dominates the loads: they take its value. An unpromoted
/// alloca keeps `alloca`, `store` and `load` (the write-through `Promote`
/// keeps the store).
#[test]
fn test_a_single_dominating_store_replaces_its_loads_and_goes_with_the_alloca() {
    let after = promoted(
        "define i32 @f(i32 %a, i1 %c) {
b0:
  %s = alloca i32
  store i32 %a, ptr %s
  br i1 %c, label %b1, label %b2
b1:
  %x = load i32, ptr %s
  ret i32 %x
b2:
  %y = load i32, ptr %s
  ret i32 %y
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert_eq!(after.matches("ret i32 %a").count(), 2, "{after}");
}

/// Stores and loads in one block: each load takes the store above it.
#[test]
fn test_loads_in_one_block_take_the_store_above_them() {
    let after = promoted(
        "define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  store i32 1, ptr %s
  %x = load i32, ptr %s
  store i32 %a, ptr %s
  %y = load i32, ptr %s
  %z = add i32 %x, %y
  ret i32 %z
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("add i32 1, %a"), "{after}");
}

/// Stores in the arms of a branch: a phi at the join, and nothing in memory.
#[test]
fn test_stores_in_two_arms_make_a_phi_at_the_join() {
    let after = promoted(
        "define i32 @f(i1 %c) {
b0:
  %s = alloca i32
  br i1 %c, label %b1, label %b2
b1:
  store i32 1, ptr %s
  br label %b3
b2:
  store i32 2, ptr %s
  br label %b3
b3:
  %x = load i32, ptr %s
  ret i32 %x
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(
        after.contains("phi i32 [ 1, %b1 ], [ 2, %b2 ]") || after.contains("phi i32 [ 2, %b2 ], [ 1, %b1 ]"),
        "{after}"
    );
}

/// A counter around a loop: a phi in the header carries it.
#[test]
fn test_a_loop_carried_slot_is_a_phi_in_the_header() {
    let after = promoted(
        "define i32 @f(i32 %n) {
b0:
  %i = alloca i32
  store i32 0, ptr %i
  br label %b1
b1:
  %v = load i32, ptr %i
  %c = icmp slt i32 %v, %n
  br i1 %c, label %b2, label %b3
b2:
  %w = load i32, ptr %i
  %next = add i32 %w, 1
  store i32 %next, ptr %i
  br label %b1
b3:
  %r = load i32, ptr %i
  ret i32 %r
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("phi i32"), "{after}");
}

/// An alloca whose address is passed on is not promotable: it stays, with
/// its store and its load.
#[test]
fn test_an_alloca_whose_address_escapes_stays() {
    let after = promoted(
        "declare void @sink(ptr)

define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  store i32 %a, ptr %s
  call void @sink(ptr %s)
  %x = load i32, ptr %s
  ret i32 %x
}
",
    );
    assert!(after.contains("alloca") && after.contains("store") && after.contains("load i32"), "{after}");
}

/// A volatile load is no promotion.
#[test]
fn test_a_volatile_load_keeps_its_alloca() {
    let after = promoted(
        "define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  store i32 %a, ptr %s
  %x = load volatile i32, ptr %s
  ret i32 %x
}
",
    );
    assert!(after.contains("alloca") && after.contains("load volatile"), "{after}");
}
