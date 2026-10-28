//! `memcpy`, `memmove` and `memset` of a constant length.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn function<'a>(
    asm: &'a str,
    name: &str,
) -> &'a str {
    let start = asm.find(&format!("_{name}:")).expect("function");
    let rest = &asm[start + 1..];
    &asm[start..start + 1 + rest.find("\n_").unwrap_or(rest.len())]
}

/// Every `memcpy` was a call to the C library, even of 224 bytes that clang
/// copies with seven `ldp`/`stp q`.
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

/// A `memmove` stores nothing before it has loaded everything, or an
/// overlapping source is overwritten.
#[test]
fn a_memmove_loads_all_before_it_stores() {
    let asm = driver::assembly(include_str!("data/memory_expansion.ll"), Platform::Darwin, true).expect("lowers");
    let moved = function(&asm, "move_40");
    let last_load = moved.rfind("ldr").max(moved.rfind("ldp")).expect("loads");
    let first_store = moved.find("str").min(moved.find("stp")).expect("stores");
    assert!(last_load < first_store, "{moved}");
}

/// A zero fill of 300 bytes is one `movi` and `stp q`, the last store
/// overlapping the one before it.
#[test]
fn a_constant_fill_stores_a_vector() {
    let asm = driver::assembly(include_str!("data/memory_expansion.ll"), Platform::Darwin, true).expect("lowers");
    let zero = function(&asm, "zero_300");
    assert!(zero.contains("movi.2d") && !zero.contains("_memset"), "{zero}");
    // Stores of the one register pair: nine `stp q, q` and the overlapping `str
    // q`, not 19 `str q`.
    assert_eq!(zero.matches("stp q").count(), 9, "{zero}");
    let fill = function(&asm, "fill_20");
    assert!(fill.contains("dup.16b") && !fill.contains("_memset"), "{fill}");
}

/// A halfword load of `@g + 1` was `ldrh w0, [x8, _g@PAGEOFF+1]`: the
/// relocation scales the offset by two, so the linker kept `(1 >> 1) = 0` and
/// the load read `@g`.
#[test]
fn an_odd_offset_in_a_global_is_not_a_scaled_page_offset() {
    let asm = driver::assembly(include_str!("data/memory_expansion.ll"), Platform::Darwin, true).expect("lowers");
    let odd = function(&asm, "odd_global");
    let scaled = odd.lines().any(|line| line.trim_start().starts_with("ld") && line.contains("PAGEOFF+1"));
    assert!(!scaled, "{odd}");
}
