//! The unroller sizes a loop with `getInstructionCost`; a vector `icmp` and a
//! `zext` of its mask had no price there, so the loop "could not be sized" and
//! stayed rolled. pattern_fill -O2 (the sums of `a` and `b` inside a 6-trip
//! loop): 1,714 instructions run where the pin, which unrolls the 4-trip
//! vector loop, runs 1,492 (clang) and ours now 1,471.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// The vector loop over 64 elements is unrolled: only the 6-trip loop around
/// it keeps a conditional branch (two before: the vector loop's too).
#[test]
fn a_vector_loop_with_a_compare_and_a_mask_extension_is_unrolled() {
    let asm = driver::assembly(include_str!("data/vector_compare_unroll.ll"), Platform::Linux, Level::O2)
        .expect("lowers");
    let branches = asm.lines().filter(|line| line.starts_with("\tb.")).count();
    assert_eq!(branches, 1, "{asm}");
}
