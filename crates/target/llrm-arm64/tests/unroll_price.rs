//! Full unrolls are priced by the estimate at 1 clock per byte (UP1).

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
/// the init loops unroll: fewer than the 98 instructions without it. Counted in
/// instructions: a sunk constant's edge block adds a label, not an instruction.
/// The exact count follows block order (93 before the pin's placement of phi
/// edge blocks, 94 after; the pin's `opt -O2 | llc -O2` on the same IR: 103).
/// #9060 (`biasPhysReg`, the pin's order for a copy of a physical register)
/// put it at 104, one over the pin, with both loops still unrolled.
#[test]
fn init_loops_whose_stores_merge_when_unrolled_are_unrolled() {
    let insns = instructions(include_str!("data/bench_lru.ll"), Level::O2);
    assert!(insns <= 104, "{insns} instructions");
}

/// crc -O2: the 8-trip bit loop inside the 9-trip byte loop saves 207 clocks
/// for 112 B (1.8 clocks a byte): unrolled, 152 B and 262 instructions where the
/// rolled loop is 88 B and 494 (clang 271, gcc 238). Unrolled it has 4 labels,
/// rolled 6.
#[test]
fn a_nest_loop_that_saves_more_than_its_bytes_cost_is_unrolled() {
    assert_eq!(labels(include_str!("data/bench_crc.ll")), 4);
}

fn instructions(source: &str, level: Level) -> usize {
    let asm = driver::assembly(source, Platform::Linux, level).expect("lowers");
    asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count()
}

/// matmul -O2: the multiply loops save 2,112 clocks for 92 B and unroll; the
/// init loops that fill `a` and `b` save 18 clocks for 17,546 B and stay
/// rolled. 338 instructions (all unrolled: more than a thousand).
#[test]
fn a_nest_is_unrolled_where_it_pays_and_not_where_it_does_not() {
    let insns = instructions(include_str!("data/bench_matmul.ll"), Level::O2);
    assert!(insns < 400, "{insns} instructions");
}

/// matmul -O3: loops carried over from an earlier slot were unrolled unpriced
/// at -O3 too: 700 -> 1,208 B. The pin prices them there: the module stays
/// under 250 instructions (ungated: 311, gated: 184).
#[test]
fn loops_carried_between_slots_are_priced_at_o3() {
    let insns = instructions(include_str!("data/bench_matmul.ll"), Level::O3);
    assert!(insns < 250, "{insns} instructions");
}

/// nbody -O2: the last update loop is priced while the pair nest's copies are
/// still rolled loops made by an earlier slot of the loop pass manager
/// (#8054 runs the pass once for each loop). They were taken for readers that
/// stay rolled, the array for one that stays in memory, and the update loop
/// kept rolled: a `sub sp, sp, #80` frame for the array. Decided with their
/// nest, the array is in registers and no local frame is made.
///
/// Not counted in instructions: the scheduler's pressure moves the count
/// (116 to 135 with #8961, which spills as the pin does: the pin's llc on the
/// same IR makes 136, ours 135) while the unroll decision stays (10 labels,
/// 72 FP operations in every state).
#[test]
fn loops_an_earlier_slot_unrolled_around_are_decided_with_their_nest() {
    let asm = driver::assembly(include_str!("data/bench_nbody.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert!(!asm.lines().any(|line| line.starts_with("\tsub sp, sp")), "the array stays in memory:\n{asm}");
}

/// nbody_single -O2: the 6-trip loops sit in a 1000-trip loop and were priced
/// as if it ran 32 times: 17,408 clocks saved against 72,250 for the bytes,
/// kept rolled: 737,170 instructions where unrolled they are 501,176. Rolled
/// the module is 112 instructions, unrolled 190 (183 since #9232's symbol bases).
#[test]
fn a_loop_in_a_known_thousand_trip_loop_is_priced_by_its_real_runs() {
    assert!(instructions(include_str!("data/bench_nbody_single.ll"), Level::O2) >= 150);
}

/// stft_fixed -O2: the 8-trip bit-reverse of the outer counter folds to a
/// constant per outer trip; priced alone it was kept rolled (+12,544
/// instructions and +28 B over unrolled). Rolled it has 29 labels, unrolled 27.
#[test]
fn a_loop_whose_copies_make_data_constant_is_unrolled_with_its_nest() {
    assert!(labels(include_str!("data/bench_stft_fixed.ll")) <= 27);
}

/// tile -O2: the 40-trip loop inside a 25-trip loop saves 3,000 clocks for 300
/// B (10 a byte): unrolled, 364 B and 2,155 instructions run where the rolled
/// loop is 68 B and 4,181 (gcc 80 B, 4,184). The module is 115 instructions;
/// rolled, fewer than 80.
#[test]
fn a_loop_that_saves_ten_clocks_a_byte_is_unrolled() {
    let insns = instructions(include_str!("data/bench_tile.ll"), Level::O2);
    assert!(insns >= 100, "{insns} instructions");
}

/// jpeg_fixed -O2: a 16-trip dot product inside two loops of unknown count was
/// unrolled (+104 B) for a saving counted over 32 x 32 assumed runs of the
/// loops around it (49,152 clocks); the program's instructions did not move.
/// An unknown loop around counts once. Rolled: the loop's label stays.
#[test]
fn a_loop_inside_unknown_loops_is_not_unrolled_for_runs_nobody_counted() {
    assert!(labels(include_str!("data/unroll_cold_nest.ll")) >= 6);
}
