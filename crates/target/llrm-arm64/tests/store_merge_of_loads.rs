//! Three byte copies were three loads and three stores (the store merge of loads found no legal width for a
//! register-less `i16`: the pin's `TypePromoteInteger` branch of `tryStoreMergeOfLoads` was missing); the pin
//! makes two bytes one extending halfword load and one truncating halfword store.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn three_byte_copies_are_a_byte_and_a_halfword_copy() {
    let asm = driver::assembly(include_str!("data/copy3.ll"), Platform::Linux, Level::O2).expect("lowers");
    let memory = asm
        .lines()
        .map(str::trim)
        .filter(|line| ["ldr", "ldur", "str", "stur"].iter().any(|op| line.split_whitespace().next().is_some_and(|word| word.starts_with(op))))
        .count();
    assert_eq!(memory, 4, "{asm}");
    assert!(asm.contains("ldurh") || asm.contains("ldrh"), "{asm}");
}
