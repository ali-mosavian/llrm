//! A narrow counter whose chain is masked and extended is widened.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn assembly(source: &str) -> String {
    driver::assembly(source, Platform::Linux, Level::O2).expect("lowers")
}

/// ring -O2: `(i*5+3)&1023` on a `short i` stayed an i16 chain LSR does not
/// see: the multiply and add ran every trip, 52,105 instructions to gcc's
/// 41,128. Widened and reduced, the index steps by a constant and no trip
/// computes `i*5`. The module was 26 instructions rolled; LV interleaves the
/// sum four ways (50), which the pin leaves scalar with the chain unreduced.
#[test]
fn a_short_counter_masked_into_an_index_is_widened_and_reduced() {
    let asm = assembly(include_str!("data/bench_ring.ll"));
    assert!(asm.contains("and x"), "premise: the index is masked in 64 bits:\n{asm}");
    let chain: Vec<&str> = asm.lines().filter(|line| line.starts_with("\tadd w") && line.ends_with("lsl #2")).collect();
    assert!(chain.is_empty(), "the `i*5` chain runs every trip: {chain:?}\n{asm}");
}
