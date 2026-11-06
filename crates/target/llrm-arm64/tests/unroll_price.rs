//! Full unrolls are priced by the estimate at 250 clocks per byte.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn labels(source: &str) -> usize {
    let asm = driver::assembly(source, Platform::Linux, Level::O2).expect("lowers");
    asm.lines().filter(|line| line.starts_with(".L")).count()
}

/// nbody -O2: the triangular nest's inner copies were priced alone and kept
/// rolled, 38,049 instructions where the pin runs 14,233. The unrolled nest
/// has 10 labels.
#[test]
fn a_nest_whose_inner_count_follows_the_outer_counter_is_the_pins_decision() {
    assert_eq!(labels(include_str!("data/bench_nbody_nest.ll")), 10);
}

/// lru -O2: the estimate priced each copy's constant stores one by one
/// (+28 B), vetoed both init loops, and ran 88 instructions / 364 B against the
/// pin's 80 / 332. Adjacent constant stores merge into stp; with the credit
/// the init loops unroll: 11 labels, 93 instructions (without it 15 and 98).
#[test]
fn init_loops_whose_stores_merge_when_unrolled_are_unrolled() {
    assert_eq!(labels(include_str!("data/bench_lru.ll")), 11);
}
