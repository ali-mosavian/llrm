//! llrm-core's `analysis/observers_tests.rs`, written as rich MIR. What
//! `avail::dead_stores` finds dead with `private`, and without it.
//!
//! Skipped, reading BC fixtures:
//! `test_a_taken_frame_address_makes_no_frame_cell_private`,
//! `test_only_the_main_body_owns_a_variable_nothing_else_names`, the three
//! nbody tests and `test_a_long_handed_to_a_sub_keeps_both_halves_stored`.

use llrm_support::hash::IndexMap;

use super::private;
use crate::alias;
use crate::avail::dead_stores;
use crate::memory::Unit;
use crate::memoryssa::Accesses;
use crate::testing::{DOS, function, layout, parsed};

/// Each store's block and index.
type Sites = Vec<(String, usize)>;

/// `body`'s @f: its dead stores, with `private` and without.
fn dead(body: &str) -> (Sites, Sites) {
    let module = parsed(&format!("{DOS}@g = global i16 0\ndeclare void @h(ptr)\ndeclare void @anything()\n\n{body}"));
    let layout = layout(&module);
    let unit = crate::testing::with_registers(Unit::of(&module, &layout, function(&module, "f")));
    let accesses = Accesses::resolved(&unit, &IndexMap::default()).unwrap();
    let pointers = alias::points_to(&unit, None, None).unwrap();
    let placed = |found: Vec<_>| {
        found
            .into_iter()
            .map(|inst| {
                let at = unit.function.parent(inst).unwrap();
                (
                    unit.function.block(at).name.clone().unwrap(),
                    unit.function.block(at).instructions().iter().position(|&one| one == inst).unwrap(),
                )
            })
            .collect::<Vec<_>>()
    };
    let private = private(unit, &pointers);
    (placed(dead_stores(&unit, &accesses, Some(&private))), placed(dead_stores(&unit, &accesses, None)))
}

fn at(
    name: &str,
    index: usize,
) -> Sites {
    vec![(name.to_owned(), index)]
}

/// nbody wrote deltaX on every inner pass: the back edge vetoed the proof.
#[test]
fn test_a_store_no_iteration_reads_is_dead_inside_its_loop() {
    let found = dead(
        "define void @f(i1 %c, i1 %d) {
b0:
  %x = alloca i16
  br label %b1

b1:
  br i1 %c, label %b2, label %b3

b2:
  store i16 1, ptr %x
  br label %b3

b3:
  br i1 %d, label %b1, label %b4

b4:
  ret void
}
",
    );
    assert_eq!(found, (at("b2", 0), vec![]));
}

#[test]
fn test_a_store_the_next_iteration_reads_stays() {
    let found = dead(
        "define void @f(i1 %c) {
b0:
  %x = alloca i16
  br label %b1

b1:
  %v = load i16, ptr %x
  store i16 %v, ptr %x
  br i1 %c, label %b1, label %b2

b2:
  ret void
}
",
    );
    assert_eq!(found, (vec![], vec![]));
}

/// Nor can the caller, but for an address that escaped.
#[test]
fn test_a_call_cannot_read_a_private_cell() {
    let body =
        |call: &str| format!("define ptr @f() {{\nb0:\n  %x = alloca i16\n  store i16 1, ptr %x\n  {call}\n}}\n");
    assert_eq!(dead(&body("call void @anything()\n  ret ptr null")), (at("b0", 1), vec![]));
    for escaping in [
        "call void @h(ptr %x)\n  ret ptr null",
        "store ptr %x, ptr @g\n  call void @anything()\n  ret ptr null",
        "ret ptr %x",
    ] {
        assert_eq!(dead(&body(escaping)), (vec![], vec![]), "{escaping}");
    }
}

/// A pointer stored into an object a call reads is read by the call:
/// `nocapture` keeps the call from keeping the address, not from loading the
/// pointer and reading what it points to. A view's descriptor holds the
/// data's pointer; the array's earlier store was dead.
#[test]
fn test_a_call_reads_what_a_pointer_stored_in_its_argument_points_to() {
    let body = |call: &str| {
        format!(
            "declare void @reads(ptr nocapture readonly)\ndefine void @f() {{\nb0:\n  %x = alloca i16\n  %holder = alloca ptr\n  store i16 1, ptr %x\n  store ptr %x, ptr %holder\n  {call}\n  ret void\n}}\n"
        )
    };
    assert_eq!(dead(&body("call void @reads(ptr %holder)")), (vec![], vec![]));
    // A call that is given nothing of it reads nothing of it.
    assert_eq!(dead(&body("call void @anything()")), (vec![("b0".to_owned(), 2), ("b0".to_owned(), 3)], vec![]));
}

#[test]
fn test_an_unresolved_address_cannot_read_a_private_cell_but_its_name_can() {
    let body = |read: &str| {
        format!(
            "define i16 @f(ptr %p) {{\nb0:\n  %x = alloca i16\n  store i16 1, ptr %x\n  %v = load i16, ptr {read}\n  ret i16 %v\n}}\n"
        )
    };
    assert_eq!(dead(&body("%p")), (at("b0", 1), vec![]));
    assert_eq!(dead(&body("%x")), (vec![], vec![]));
}

/// A global outlives the function.
#[test]
fn test_a_global_is_never_private() {
    assert_eq!(
        dead("define void @f() {\nb0:\n  store i16 1, ptr @g\n  call void @anything()\n  ret void\n}\n"),
        (vec![], vec![])
    );
}
