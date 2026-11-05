//! A full unroll at -O2 is priced on the unrolled, cleaned copy of the function
//! (DEVIATE UP1 trial pricing), not by an estimate of what the analysis folds.
//! nbody -O2 kept a 3-trip loop rolled (144 instructions): its estimate priced
//! the copies at +14 B where unrolling and cleaning shrinks the code 73 B (116).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn instructions(source: &str) -> usize {
    let asm = driver::assembly(source, Platform::Linux, Level::O2).expect("lowers");
    asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count()
}

#[test]
fn a_loop_whose_unroll_shrinks_the_code_is_unrolled() {
    let count = instructions(include_str!("data/bench_nbody.ll"));
    assert!(count <= 125, "{count} instructions");
}

/// Guard: matmul's 8-trip loops add 155 B for 90 clocks and stay rolled.
#[test]
fn a_loop_whose_unroll_grows_the_code_for_few_clocks_stays_rolled() {
    let count = instructions(include_str!("data/bench_matmul.ll"));
    assert!(count <= 205, "{count} instructions");
}
