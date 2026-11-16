//! A loop of `llvm.smax` and `llvm.smin` calls was not vectorized: the
//! widener took `llvm.fmuladd` and no other call, so every clamp loop (the
//! `clamp` of jpeg_fixed's `convert`, `x < 0 ? 0 : x > 255 ? 255 : x`) stayed
//! scalar.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_clamp_loop_is_vectorized_with_vector_smax_and_smin() {
    let asm = driver::assembly(include_str!("data/vector_clamp.ll"), Platform::Linux, Level::O2).expect("lowers");
    let count = |prefix: &str| asm.lines().filter(|line| line.starts_with(prefix)).count();
    // The `smin` of a value the `smax` made non-negative is a `umin`.
    assert!(count("\tsmax") > 0 && count("\tumin") + count("\tsmin") > 0, "{asm}");
    assert_eq!(count("\tcmp"), 0, "a scalar clamp is a compare and a select: {asm}");
}
