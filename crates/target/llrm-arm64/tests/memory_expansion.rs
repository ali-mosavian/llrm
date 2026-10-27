//! `memcpy`, `memmove` and `memset` of a constant length.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn function<'a>(asm: &'a str, name: &str) -> &'a str {
    let start = asm.find(&format!("_{name}:")).expect("function");
    let rest = &asm[start + 1..];
    &asm[start..start + 1 + rest.find("\n_").unwrap_or(rest.len())]
}

/// Every `memcpy` was a call to the C library, even of 224 bytes that clang copies with seven `ldp`/`stp q`.
#[test]
fn a_constant_copy_within_budget_is_made_of_q_pairs() {
    let asm = driver::assembly(include_str!("data/memory_expansion.ll"), Platform::Darwin, true).expect("lowers");
    let copy = function(&asm, "copy_224");
    assert!(!copy.contains("_memcpy"), "{copy}");
    assert_eq!(copy.matches("ldp q").count(), 7, "{copy}");
    assert_eq!(copy.matches("stp q").count(), 7, "{copy}");
    // One access over the budget of 16 stays a call.
    assert!(function(&asm, "copy_257").contains("_memcpy"));
}

/// A `memmove` stores nothing before it has loaded everything, or an overlapping source is overwritten.
#[test]
fn a_memmove_loads_all_before_it_stores() {
    let asm = driver::assembly(include_str!("data/memory_expansion.ll"), Platform::Darwin, true).expect("lowers");
    let moved = function(&asm, "move_40");
    let last_load = moved.rfind("ldr").max(moved.rfind("ldp")).expect("loads");
    let first_store = moved.find("str").min(moved.find("stp")).expect("stores");
    assert!(last_load < first_store, "{moved}");
}

/// A zero fill of 300 bytes is one `movi` and `stp q`, the last store overlapping the one before it.
#[test]
fn a_constant_fill_stores_a_vector() {
    let asm = driver::assembly(include_str!("data/memory_expansion.ll"), Platform::Darwin, true).expect("lowers");
    let zero = function(&asm, "zero_300");
    assert!(zero.contains("movi.2d") && !zero.contains("_memset"), "{zero}");
    let fill = function(&asm, "fill_20");
    assert!(fill.contains("dup.16b") && !fill.contains("_memset"), "{fill}");
}
