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

/// crc -O2: the 8-trip bit loop inside the 9-trip byte loop was unrolled
/// through the nest bypass (152 B, 262 instructions) where the rolled loop is
/// 88 B and 494 instructions; 250 clocks a byte prices the 64 B at 16,000.
/// Nothing in the bit loop depends on the byte counter, so there is nothing
/// for unrolling the nest to fold. Unrolled it has 4 labels, rolled 6.
#[test]
fn a_nest_loop_that_uses_no_outer_counter_is_priced_not_bypassed() {
    assert_eq!(labels(include_str!("data/bench_crc.ll")), 6);
}

fn instructions(source: &str, level: Level) -> usize {
    let asm = driver::assembly(source, Platform::Linux, level).expect("lowers");
    asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count()
}

/// matmul -O2: the 8 x 8 x 8 nest reads its counters only to index memory,
/// and the nest bypass unrolled it anyway: 202 instructions where gcc's rolled
/// nest is 89 (+452 B over gcc). An index sum folds into the copies' own
/// offsets, not into work saved, so the loops are priced: 99 instructions.
#[test]
fn a_nest_whose_counters_only_index_memory_is_priced_not_bypassed() {
    let insns = instructions(include_str!("data/bench_matmul.ll"), Level::O2);
    assert!(insns < 130, "{insns} instructions");
}
