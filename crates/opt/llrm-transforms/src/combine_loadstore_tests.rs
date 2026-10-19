//! `combine_loadstore` against `visitStoreInst` at the pin: what makes a store
//! vanish, and what keeps it.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitStoreInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Store { .. } => c.visit_store(inst),
        _ => Visit::None,
    }
}

/// `@f`'s body (between the braces' first block label and the end) after
/// `visitStoreInst` alone, through the driver.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

fn stores(text: &str) -> usize {
    combined(text).matches("store ").count()
}

/// A store to an alloca nothing else reads was kept: the alloca stayed live.
#[test]
fn a_store_to_a_one_use_alloca_is_erased() {
    let out = combined(
        "define void @f(i16 %x) {
b0:
  %a = alloca i16
  store i16 %x, ptr %a
  ret void
}
",
    );
    assert!(!out.contains("store"), "{out}");
}

/// A store through a GEP of a one-use alloca was kept.
#[test]
fn a_store_through_a_gep_of_a_one_use_alloca_is_erased() {
    let out = combined(
        "define void @f(i16 %x) {
b0:
  %a = alloca [2 x i16]
  %g = getelementptr [2 x i16], ptr %a, i16 0, i16 1
  store i16 %x, ptr %g
  ret void
}
",
    );
    assert!(!out.contains("store"), "{out}");
}

/// An alloca that is read stays written.
#[test]
fn a_store_to_a_read_alloca_stays() {
    let n = stores(
        "define i16 @f(i16 %x) {
b0:
  %a = alloca i16
  store i16 %x, ptr %a
  %v = load i16, ptr %a
  ret i16 %v
}
",
    );
    assert_eq!(n, 1);
}

/// The store overwritten a few instructions later was kept: two stores stood
/// where one does.
#[test]
fn an_overwritten_store_is_erased() {
    let out = combined(
        "define void @f(ptr %p, i16 %a) {
b0:
  store i16 1, ptr %p
  %x = add i16 %a, 1
  store i16 %x, ptr %p
  ret void
}
",
    );
    assert_eq!(out.matches("store ").count(), 1, "{out}");
    assert!(!out.contains("store i16 1,"), "{out}");
}

/// A load between the two stores may read the first.
#[test]
fn a_load_between_two_stores_keeps_both() {
    let n = stores(
        "define i16 @f(ptr %p, ptr %q) {
b0:
  store i16 1, ptr %p
  %v = load i16, ptr %q
  store i16 2, ptr %p
  ret i16 %v
}
",
    );
    assert_eq!(n, 2);
}

/// The scan looks at 6 instructions: the store 7 back is kept.
#[test]
fn the_scan_stops_after_six_instructions() {
    // A chain, so the driver does not erase the adds as dead.
    let text = |n: usize| {
        let chain: String = (0..n)
            .map(|i| {
                format!("  %t{i} = add i16 {}, 1\n", if i == 0 { "%a".to_owned() } else { format!("%t{}", i - 1) })
            })
            .collect();
        format!(
            "define i16 @f(ptr %p, i16 %a) {{\nb0:\n  store i16 1, ptr %p\n{chain}  store i16 2, ptr %p\n  ret i16 %t{}\n}}\n",
            n - 1
        )
    };
    assert_eq!(stores(&text(5)), 1);
    assert_eq!(stores(&text(6)), 2);
}

/// `%x = load %p; store %x, %p` writes what is there.
#[test]
fn a_store_of_the_value_just_loaded_is_erased() {
    let out = combined(
        "define i16 @f(ptr %p) {
b0:
  %x = load i16, ptr %p
  store i16 %x, ptr %p
  ret i16 %x
}
",
    );
    assert!(!out.contains("store"), "{out}");
}

/// A store of `undef` stored it: a write the next read cannot tell from none.
#[test]
fn a_store_of_undef_is_erased() {
    for value in ["undef", "poison"] {
        let out = combined(&format!("define void @f(ptr %p) {{\nb0:\n  store i16 {value}, ptr %p\n  ret void\n}}\n"));
        assert!(!out.contains("store"), "{value}: {out}");
    }
}

/// A store to null writes poison: SimplifyCFG's `unreachable` marker.
#[test]
fn a_store_to_null_stores_poison() {
    for (setup, address) in [("", "null"), ("  %g = getelementptr i16, ptr null, i16 %y\n", "%g")] {
        let out = combined(&format!(
            "define void @f(i16 %x, i16 %y) {{\nb0:\n{setup}  store i16 %x, ptr {address}\n  ret void\n}}\n"
        ));
        assert!(out.contains("store i16 poison"), "{address}: {out}");
    }
}

/// A volatile store is left alone.
#[test]
fn a_volatile_store_stays() {
    let n = stores(
        "define void @f(i16 %x) {
b0:
  %a = alloca i16
  store volatile i16 %x, ptr %a
  ret void
}
",
    );
    assert_eq!(n, 1);
}
