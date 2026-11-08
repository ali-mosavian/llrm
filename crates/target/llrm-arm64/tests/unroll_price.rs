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
/// nest is 89. Its multiply loops are priced; loops that store a computed
/// constant still unroll. Fewer than 190.
#[test]
fn a_nest_whose_counters_only_index_memory_is_priced_not_bypassed() {
    let insns = instructions(include_str!("data/bench_matmul.ll"), Level::O2);
    assert!(insns < 190, "{insns} instructions");
}

/// matmul -O2: the init nest fills `a` and `b` with a computed value, and the
/// `k` loop reads them at run-time indices. The array stays in memory, so the
/// stores were credited as promoted for nothing, and the inner loop was
/// unrolled alone by the nest bypass while the outer was kept rolled:
/// 174 instructions (660 B), 292 B rolled. Fewer than 120.
#[test]
fn an_init_nest_whose_arrays_stay_in_memory_is_not_unrolled() {
    let insns = instructions(include_str!("data/bench_matmul.ll"), Level::O2);
    assert!(insns < 120, "{insns} instructions");
}

/// matmul and tile -O3: the nest was left to the pin's bar at -O3, which unrolls
/// the 8 x 8 x 8 nest (700 B) and tile's 40-trip loop (364 B) where the byte
/// rate keeps them rolled (292 B and 68 B). The rate holds at -O3.
#[test]
fn a_nest_is_priced_at_o3_as_at_o2() {
    let ir = include_str!("data/bench_matmul.ll");
    let (o2, o3) = (instructions(ir, Level::O2), instructions(ir, Level::O3));
    assert!(o3 < o2 + 20, "{o3} instructions at -O3, {o2} at -O2");
    let tile = include_str!("data/bench_tile.ll");
    assert!(instructions(tile, Level::O3) < 80, "tile -O3: {} instructions", instructions(tile, Level::O3));
}

/// A synthetic 8 x 8 nest fills a local array that a loop of run-time trips
/// reads at run-time indices (`data/nest_fill_then_read.c`). The array stays in
/// memory, so unrolling the fill saves nothing it keeps: the module is the same
/// size at -O3 as at -O2 (46 instructions). -O3 unrolled the inner fill loop
/// first, its nest bypass assuming the nest would follow: 69.
#[test]
fn a_nest_filling_an_array_read_at_run_time_indices_is_rolled_at_every_level() {
    let ir = include_str!("data/nest_fill_then_read.ll");
    let (o2, o3) = (instructions(ir, Level::O2), instructions(ir, Level::O3));
    assert!(o3 <= o2 + 5, "{o3} instructions at -O3, {o2} at -O2");
}

/// nbody_single -O2: the 6-trip loops sit in a 1000-trip loop and were priced
/// as if it ran 32 times: 17,408 clocks saved against 72,250 for the bytes,
/// kept rolled: 737,170 instructions where unrolled they are 501,176. Rolled
/// the module is 112 instructions, unrolled 190.
#[test]
fn a_loop_in_a_known_thousand_trip_loop_is_priced_by_its_real_runs() {
    assert!(instructions(include_str!("data/bench_nbody_single.ll"), Level::O2) >= 185);
}

/// stft_fixed -O2: the 8-trip bit-reverse of the outer counter folds to a
/// constant per outer trip; priced alone it was kept rolled (+12,544
/// instructions and +28 B over unrolled). Rolled it has 29 labels, unrolled 27.
#[test]
fn a_loop_whose_copies_make_data_constant_is_unrolled_with_its_nest() {
    assert!(labels(include_str!("data/bench_stft_fixed.ll")) <= 27);
}

/// tile -O2: the 40-trip loop inside a 25-trip loop was unrolled by the nest
/// bypass: the outer counter's only reader is the address of a load, which is
/// not data the nest folds. 364 B and 2,155 instructions where the rolled loop
/// is 68 B and 4,181 (gcc 80 B, 4,184). The module was 115 instructions;
/// fewer than 80 with the loop priced.
#[test]
fn a_nest_loop_whose_outer_counter_only_addresses_a_load_is_priced_not_bypassed() {
    let insns = instructions(include_str!("data/bench_tile.ll"), Level::O2);
    assert!(insns < 80, "{insns} instructions");
}
