//! The unroller sized a loop with `getInstructionCost`; a vector `mul` of
//! extended operands (`smull`) and the `llvm.vector.reduce.add` of a
//! vectorized dot product had no price there, so the dot loop could not be
//! sized and stayed rolled, and the arrays it reads were never folded to
//! constants. bench dot -O2: 3,209 instructions where clang runs 739; with the
//! loop sized, 838.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// The three loops that fill and read `a` and `b` unroll and fold; only the
/// 40-trip loop around them keeps a conditional branch (two before: the
/// vectorized dot loop's too).
#[test]
fn a_vectorized_dot_product_is_sized_and_unrolled() {
    let asm = driver::assembly(include_str!("data/vector_dot_unroll.ll"), Platform::Linux, Level::O2).expect("lowers");
    let kernel: Vec<&str> =
        asm.lines().skip_while(|line| *line != "bench_dot:").take_while(|line| *line != "main:").collect();
    let branches = kernel.iter().filter(|line| line.starts_with("\tb.")).count();
    assert_eq!(branches, 1, "{}", kernel.join("\n"));
}
