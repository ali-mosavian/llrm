//! `(add (add x, K1), (add y, K2))` of build-vector constants was never
//! regrouped: `isConstantIntBuildVectorOrConstantInt` knew scalars only and
//! `FoldConstantArithmetic` folded no vector lanes, so the four constant
//! addends of a widened multiply-and-reduce each became the accumulator of a
//! `smlal` (a `mov vN.16b` copy and a constant load each: 33 instructions);
//! summed once at compile time, two `smull`, two `smlal` and one add.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn constant_addends_of_a_reduce_tree_are_summed_at_compile_time() {
    let asm =
        driver::assembly(include_str!("data/vector_const_reduce.ll"), Platform::Linux, Level::O2).expect("lowers");
    let kernel: Vec<&str> =
        asm.lines().skip_while(|line| *line != "f:").take_while(|line| !line.starts_with("\tret")).collect();
    let count = |mnemonic: &str| kernel.iter().filter(|line| line.starts_with(mnemonic)).count();
    assert_eq!(count("\tsmull "), 2, "{}", kernel.join("\n"));
    assert_eq!(count("\tmov v"), 0, "{}", kernel.join("\n"));
}
