//! `reaching` on the shapes of a spilled pointer: the stores that supply a load
//! of a slot nothing else writes.

use llrm_mir::dense::IdSet;
use llrm_mir::module::{InstId, Module};
use llrm_mir::opcode::Opcode;

use crate::memory::Unit;
use crate::testing::{DOS, function, layout, parsed, value};

/// The stores of `@f` that reach the loads named `names` (by result), each as
/// the stored value's name, sorted; none where the load has no exact source.
fn supplied(
    text: &str,
    names: &[&str],
) -> Vec<Option<Vec<String>>> {
    let module: Module = parsed(&format!("{DOS}{text}"));
    let layout = layout(&module);
    let f = function(&module, "f");
    let unit = crate::testing::with_registers(Unit::of(&module, &layout, f));
    let loads: IdSet<InstId> = names
        .iter()
        .map(|name| {
            let result = value(f, name);
            f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(result)).expect("a load")
        })
        .collect();
    let found = super::reaching(&unit, &loads);
    names
        .iter()
        .map(|name| {
            let result = value(f, name);
            let load = f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(result))?;
            let stores = found.get(&load)?;
            let mut named: Vec<String> = stores
                .iter()
                .map(|&store| {
                    assert!(matches!(f.instruction(store).opcode, Opcode::Store { .. }));
                    match f.instruction(store).operands[0] {
                        llrm_mir::module::Operand::Value(one) => f.value(one).name.clone().unwrap_or_default(),
                        _ => String::new(),
                    }
                })
                .collect();
            named.sort();
            Some(named)
        })
        .collect()
}

#[test]
fn a_load_after_one_store_is_supplied_by_it() {
    let got = supplied(
        "define void @f(ptr %p) {
b0:
  %slot = alloca ptr
  store ptr %p, ptr %slot
  %r = load ptr, ptr %slot
  ret void
}
",
        &["r"],
    );
    assert_eq!(got, [Some(vec!["p".to_owned()])]);
}

#[test]
fn a_join_of_two_stores_is_supplied_by_both_and_a_join_with_one_path_unstored_by_none() {
    let both = "define void @f(i1 %c, ptr %p, ptr %q) {
b0:
  %slot = alloca ptr
  br i1 %c, label %b1, label %b2
b1:
  store ptr %p, ptr %slot
  br label %b3
b2:
  store ptr %q, ptr %slot
  br label %b3
b3:
  %r = load ptr, ptr %slot
  ret void
}
";
    assert_eq!(supplied(both, &["r"]), [Some(vec!["p".to_owned(), "q".to_owned()])]);
    let one = "define void @f(i1 %c, ptr %p) {
b0:
  %slot = alloca ptr
  br i1 %c, label %b1, label %b3
b1:
  store ptr %p, ptr %slot
  br label %b3
b3:
  %r = load ptr, ptr %slot
  ret void
}
";
    assert_eq!(supplied(one, &["r"]), [None], "the path that skips the store reads nothing stored");
}

#[test]
fn a_store_in_a_loop_reaches_a_load_after_it_and_not_one_before_the_first_trip() {
    let text = "define void @f(i1 %c, ptr %p, ptr %q) {
b0:
  %slot = alloca ptr
  store ptr %p, ptr %slot
  br label %b1
b1:
  %head = load ptr, ptr %slot
  store ptr %q, ptr %slot
  br i1 %c, label %b1, label %b2
b2:
  %after = load ptr, ptr %slot
  ret void
}
";
    assert_eq!(
        supplied(text, &["head", "after"]),
        [Some(vec!["p".to_owned(), "q".to_owned()]), Some(vec!["q".to_owned()])]
    );
}

#[test]
fn a_write_a_slot_may_take_ends_what_was_stored_and_one_it_cannot_does_not() {
    // `@g` writes through its argument, which is the slot's own address:
    // exposed.
    let exposed = "declare void @g(ptr)

define void @f(ptr %p) {
b0:
  %slot = alloca ptr
  store ptr %p, ptr %slot
  call void @g(ptr %slot)
  %r = load ptr, ptr %slot
  ret void
}
";
    assert_eq!(supplied(exposed, &["r"]), [None]);
    // The same call on another object leaves a slot nothing exposes alone.
    let private = "declare void @g(ptr)

define void @f(ptr %p, ptr %q) {
b0:
  %slot = alloca ptr
  store ptr %p, ptr %slot
  call void @g(ptr %q)
  %r = load ptr, ptr %slot
  ret void
}
";
    assert_eq!(supplied(private, &["r"]), [Some(vec!["p".to_owned()])]);
    // A call that writes nothing leaves even an exposed slot as it was.
    let reading = "declare void @g(ptr) memory(read)

define void @f(ptr %p) {
b0:
  %slot = alloca ptr
  store ptr %p, ptr %slot
  call void @g(ptr %slot)
  %r = load ptr, ptr %slot
  ret void
}
";
    assert_eq!(supplied(reading, &["r"]), [Some(vec!["p".to_owned()])]);
}

#[test]
fn a_store_at_an_unknown_index_or_over_part_of_the_cell_ends_what_was_stored() {
    let indexed = "define void @f(ptr %p, i16 %i) {
b0:
  %slot = alloca [4 x ptr]
  store ptr %p, ptr %slot
  %at = getelementptr ptr, ptr %slot, i16 %i
  store ptr null, ptr %at
  %r = load ptr, ptr %slot
  ret void
}
";
    assert_eq!(supplied(indexed, &["r"]), [None]);
    let part = "define void @f(ptr %p) {
b0:
  %slot = alloca ptr
  store ptr %p, ptr %slot
  %high = getelementptr i8, ptr %slot, i16 1
  store i8 0, ptr %high
  %r = load ptr, ptr %slot
  ret void
}
";
    assert_eq!(supplied(part, &["r"]), [None]);
}

/// A store through a phi or a select of two slots' addresses, or through two
/// variable indices, writes a slot that nothing exposes without naming it as
/// its root: the scan did not see it, and a load of the slot was supplied by
/// the store before (a miscompile of the stored pointer's facts).
#[test]
fn a_slot_written_through_a_joined_address_or_two_indices_has_no_exact_source() {
    let joined = "define void @f(i1 %c, ptr %p, ptr %q) {
b0:
  %a = alloca ptr
  %b = alloca ptr
  store ptr %p, ptr %a
  br i1 %c, label %l, label %r
l:
  br label %j
r:
  br label %j
j:
  %w = phi ptr [ %a, %l ], [ %b, %r ]
  store ptr %q, ptr %w
  %x = load ptr, ptr %a
  ret void
}
";
    assert_eq!(supplied(joined, &["x"]), [None]);
    let two = "define void @f(ptr %p, ptr %q, i16 %i, i16 %j) {
b0:
  %a = alloca [4 x ptr]
  store ptr %p, ptr %a
  %g = getelementptr i8, ptr %a, i16 %i
  %h = getelementptr i8, ptr %g, i16 %j
  store ptr %q, ptr %h
  %x = load ptr, ptr %a
  ret void
}
";
    assert_eq!(supplied(two, &["x"]), [None]);
}
