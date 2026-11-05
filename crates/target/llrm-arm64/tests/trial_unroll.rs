//! A full unroll at -O2 is priced on an unrolled, cleaned copy of the function
//! (DEVIATE UP1 trial pricing), not by an estimate of what the analysis folds.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn instructions(source: &str) -> usize {
    let asm = driver::assembly(source, Platform::Linux, Level::O2).expect("lowers");
    asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count()
}

/// nbody -O2: the 4-trip outer loop of a triangular nest cost +100 B in a copy
/// that unrolled it with its inner loops rolled, and was kept rolled (114
/// instructions, 38,049 executed against 14,233); the nest as the pin unrolls it
/// is 224 B smaller (149 instructions).
#[test]
fn a_nest_that_pays_whole_is_unrolled_though_the_outer_loop_alone_does_not() {
    let count = instructions(include_str!("data/bench_nbody_nest.ll"));
    assert!(count >= 140, "{count} instructions");
}
