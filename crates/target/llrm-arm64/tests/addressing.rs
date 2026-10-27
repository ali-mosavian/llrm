//! How the selector addresses globals.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

/// Three stores to one global at different constant offsets each started with their own `adrp
/// _pool@PAGE+N`: clang makes the address once and addresses the stores from it (tests/llc/list).
#[test]
fn accesses_to_one_global_in_a_block_share_its_address() {
    let text = include_str!("data/global_offsets.ll");
    let asm = driver::assembly(text, Platform::Darwin, true).expect("the function lowers");
    let adrps = asm.lines().filter(|line| line.trim().starts_with("adrp")).count();
    assert_eq!(adrps, 1, "{asm}");
}

/// Stores to two arrays, interleaved, were never paired: `pairs` stopped at any store between two, because it
/// had nothing to say two stores to different globals do not overlap (the pin's LoadStoreOptimizer asks the
/// memory operands). clang: one `stp` per array.
#[test]
fn stores_to_different_globals_pair_across_each_other() {
    let text = include_str!("data/interleaved_stores.ll");
    let asm = driver::assembly(text, Platform::Darwin, true).expect("the function lowers");
    let pairs = asm.lines().filter(|line| line.trim().starts_with("stp")).count();
    assert_eq!(pairs, 2, "{asm}");
}
