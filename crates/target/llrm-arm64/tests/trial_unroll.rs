//! A full unroll at -O2 is priced on an unrolled, cleaned copy of the function
//! (DEVIATE UP1 trial pricing), not by an estimate of what the analysis folds.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn instructions(source: &str) -> usize {
    let asm = driver::assembly(source, Platform::Linux, Level::O2).expect("lowers");
    asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count()
}

fn labels(source: &str) -> usize {
    let asm = driver::assembly(source, Platform::Linux, Level::O2).expect("lowers");
    asm.lines().filter(|line| line.starts_with(".L")).count()
}

/// nbody -O2: the triangular nest (4 trips around a loop of at most 3, whose
/// count follows the outer counter) was priced +267 B and kept rolled, 38,049
/// instructions executed where the pin's whole-nest unroll runs 14,233 for 8 B
/// less. The pin's unrolled nest has 10 labels; with the nest kept rolled, or
/// unrolled by a trial, it has fewer.
#[test]
fn a_nest_whose_inner_count_follows_the_outer_counter_is_the_pins_decision() {
    assert_eq!(labels(include_str!("data/bench_nbody_nest.ll")), 10);
}

/// Guard: matmul's 8-trip loops are constant whatever their outer loops do, so
/// they are priced on their own and stay rolled (202 instructions).
#[test]
fn a_loop_of_constant_count_around_the_nest_is_still_priced() {
    assert!(instructions(include_str!("data/bench_matmul.ll")) <= 205);
}
