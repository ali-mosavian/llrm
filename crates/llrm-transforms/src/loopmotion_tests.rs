//! Adapted from llrm-core's `optimize/loopmotion_tests.rs`, the port of
//! `tests/test_loopmotion.py`, each body now MIR text run by llrm-mir's
//! interpreter before and after.
//!
//! HOTLOP's counter is a global the header stores. Its observers are a
//! load, a store, a call and a volatile access; `escape` and `opaque` have
//! no rich MIR kind. Skipped, BC and OMF fixtures through the old pipeline:
//! `test_nested_accumulator_is_stored_only_after_the_outer_loop`,
//! `test_indexed_record_accumulators_store_only_after_loop` and
//! `test_a_float_loop_sinks_its_counter_store_without_an_error_handler`.

use std::rc::Rc;

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::{Analyses, Outer};

use super::sunk_stores;
use crate::testing::{parsed, printed, results};

/// `text`'s @f with its stores sunk: its printed form, and whether any
/// moved. @f computes what it did on `inputs`.
fn sunk(text: &str, inputs: &[&[i128]]) -> (String, bool) {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let (layout, outer) = (layout(&module), Rc::new(Outer::of(&module, None)));
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = sunk_stores(context, &layout, &callees, function, &mut Analyses::new(outer)).unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (after, changed)
}

fn kept(text: &str, inputs: &[&[i128]]) {
    let (after, changed) = sunk(text, inputs);
    assert!(!changed, "{after}");
    assert_eq!(after, printed(&parsed(&format!("{DOS}{text}"))));
}

const TRIPS: &[&[i128]] = &[&[0], &[1], &[50], &[-3]];

/// HOTLOP: the header stores its counter, which the loop never reads back.
/// `observer` goes in the latch.
fn hotlop(observer: &str) -> String {
    format!(
        "@n = global i16 0
@m = global i16 0

define void @touch() {{
b:
  store i16 1, ptr @m
  ret void
}}

define i16 @f(i16 %k) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  store i16 %i, ptr @n
  %c = icmp slt i16 %i, %k
  br i1 %c, label %b2, label %b3

b2:
{observer}  %next = add i16 %i, 1
  br label %b1

b3:
  %v = load i16, ptr @n
  ret i16 %v
}}
"
    )
}

#[test]
fn test_counter_is_written_once_at_the_exit_not_every_iteration() {
    let (text, changed) = sunk(&hotlop(""), TRIPS);
    assert!(changed);
    assert!(text.contains("b1:\n  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]\n  %c = icmp"), "{text}");
    assert!(text.contains("b3:\n  store i16 %i, ptr @n\n  %v = load"), "{text}");
}

#[test]
fn test_an_observer_in_the_loop_keeps_the_store() {
    for observer in ["  %o = load i16, ptr @n\n", "  store i16 3, ptr @n\n", "  call void @touch()\n", "  %o = load volatile i16, ptr @m\n"] {
        kept(&hotlop(observer), TRIPS);
    }
}

#[test]
fn test_an_exit_reachable_without_the_store_gets_no_new_write() {
    let text = hotlop("").replace("b0:\n  br label %b1", "b0:\n  %e = icmp eq i16 %k, 7\n  br i1 %e, label %b3, label %b1");
    kept(&text, &[&[0], &[1], &[7], &[50]]);
}

/// An accumulator the latch stores, and the entry seeds with `seeded`.
fn accumulator(seeded: &str) -> String {
    format!(
        "@acc = global i16 0

define i16 @f(i16 %k) {{
b0:
{seeded}  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %s = phi i16 [ 5, %b0 ], [ %s1, %b2 ]
  %c = icmp slt i16 %i, %k
  br i1 %c, label %b2, label %b3

b2:
  %s1 = add i16 %s, %i
  store i16 %s1, ptr @acc
  %next = add i16 %i, 1
  br label %b1

b3:
  %v = load i16, ptr @acc
  ret i16 %v
}}
"
    )
}

/// The cell holds the header's phi on every trip, so after the loop the
/// phi is what it holds.
#[test]
fn test_an_accumulator_stores_its_phi_once_after_the_loop() {
    let (text, changed) = sunk(&accumulator("  store i16 5, ptr @acc\n"), TRIPS);
    assert!(changed);
    assert!(text.contains("b2:\n  %s1 = add i16 %s, %i\n  %next") && text.contains("b3:\n  store i16 %s, ptr @acc\n"), "{text}");
}

#[test]
fn test_an_accumulator_without_zero_trip_initialization_stays_in_the_loop() {
    kept(&accumulator(""), TRIPS);
}

/// The latch stores `value` to @g on each of `bound` trips.
fn latch_store(value: &str, bound: &str) -> String {
    format!(
        "@g = global i16 0

define i16 @f(i16 %x) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, {bound}
  br i1 %c, label %b2, label %b3

b2:
  store i16 {value}, ptr @g
  %next = add i16 %i, 1
  br label %b1

b3:
  %v = load i16, ptr @g
  ret i16 %v
}}
"
    )
}

const VALUES: &[&[i128]] = &[&[0], &[-9], &[1234]];

/// LNGMXX: an invariant the latch stores sinks only where the loop runs.
#[test]
fn test_an_invariant_store_sinks_only_when_the_loop_runs() {
    for bound in ["1", "10"] {
        let (text, changed) = sunk(&latch_store("%x", bound), VALUES);
        assert!(changed && text.contains("b3:\n  store i16 %x, ptr @g\n"), "{text}");
    }
    kept(&latch_store("%x", "0"), VALUES);
    kept(&latch_store("%x", "%x"), VALUES);
}

/// The latch's last store of its counter is the counter's last value.
#[test]
fn test_a_stored_counter_leaves_its_last_value() {
    for (bound, last) in [("1", 0), ("10", 9)] {
        let (text, changed) = sunk(&latch_store("%i", bound), VALUES);
        assert!(changed && text.contains(&format!("b3:\n  store i16 {last}, ptr @g\n")), "{text}");
    }
    kept(&latch_store("%i", "0"), VALUES);
}

/// The header stores @a[0] while the latch loads @a[`index`].
fn indexed(index: &str) -> String {
    format!(
        "@a = global [4 x i16] zeroinitializer

define i16 @f(i16 %k, i16 %j) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  store i16 %i, ptr @a
  %c = icmp slt i16 %i, %k
  br i1 %c, label %b2, label %b3

b2:
  %p = getelementptr [4 x i16], ptr @a, i16 0, i16 {index}
  %o = load i16, ptr %p
  %next = add i16 %i, 1
  br label %b1

b3:
  %v = load i16, ptr @a
  ret i16 %v
}}
"
    )
}

const INDEXED: &[&[i128]] = &[&[0, 0], &[1, 0], &[20, 1], &[20, 3]];

#[test]
fn test_a_store_that_may_alias_a_load_stays() {
    kept(&indexed("%j"), INDEXED);
}

#[test]
fn test_a_store_apart_from_every_load_sinks() {
    let (text, changed) = sunk(&indexed("1"), INDEXED);
    assert!(changed && text.contains("b3:\n  store i16 %i, ptr @a\n"), "{text}");
}

/// An address only the latch computes does not reach the exit: moved
/// there, the store read a value its block does not dominate.
#[test]
fn test_an_address_the_latch_computes_keeps_its_store() {
    let text = latch_store("%x", "10")
        .replace("@g = global i16 0", "@a = global [4 x i16] zeroinitializer")
        .replace("  store i16 %x, ptr @g\n", "  %p = getelementptr [4 x i16], ptr @a, i16 0, i16 2\n  store i16 %x, ptr %p\n")
        .replace("load i16, ptr @g", "load i16, ptr getelementptr ([4 x i16], ptr @a, i16 0, i16 2)");
    kept(&text, VALUES);
}

/// A call before the loop that writes elsewhere leaves the seed standing.
/// Any writing call refused, and sum_three stored its total every trip
/// past a bound's runtime call.
#[test]
fn test_a_call_writing_elsewhere_keeps_the_seed() {
    let text = accumulator("  store i16 5, ptr @acc\n  call void @touch()\n").replace("define i16 @f", "@m = global i16 0\n\ndefine void @touch() {\nb:\n  store i16 1, ptr @m\n  ret void\n}\n\ndefine i16 @f");
    let module = parsed(&format!("{DOS}{text}"));
    let after = crate::testing::summarized(&module, super::LoopMotion, true, TRIPS);
    assert!(after.contains("b3:\n  store i16 %s, ptr @acc\n"), "{after}");
}
