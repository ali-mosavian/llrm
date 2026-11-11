//! Interprocedural register use on arm64 (`Tti::interprocedural_register_use`).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn asm(text: &str) -> String {
    driver::assembly(text, Platform::Linux, Level::O2).expect("lowers")
}

/// `sum` keeps its counter, bound, sum and multiplier across a call to a
/// leaf that writes two registers; the convention made them callee-saved
/// (x19-x22, a four-register save and restore around the loop).
#[test]
fn values_kept_across_a_call_to_a_known_leaf_stay_in_caller_saved_registers() {
    let text = asm(include_str!("data/ipra_leaf.ll"));
    assert!(!text.contains("x19"), "{text}");
}

/// A callee another definition may replace at link time (`weak`) is no
/// callee to price a call by: its caller keeps the convention.
#[test]
fn a_call_to_a_weak_function_keeps_the_convention() {
    let text = asm(include_str!("data/ipra_weak.ll"));
    assert!(text.contains("x19"), "{text}");
}

/// A function that calls itself settles on what it writes: `sum` keeps its
/// loop in caller-saved registers across the call to `pow`, whose own recursive
/// call is priced the same way (recpow -Os: `bench_recpow` saved six registers
/// around a call to a function writing three).
#[test]
fn a_recursive_callee_is_priced_by_what_it_writes() {
    let text = asm(include_str!("data/ipra_recursive.ll"));
    let sum = text.split("sum:").nth(1).expect("sum");
    assert!(!sum.contains("x19"), "{text}");
}
