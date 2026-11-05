//! Identical blocks that are more than a jump merge (`BranchFolder`): lru -O3
//! wrote `mov x12, xzr; mov x13, x12; b L` four times (368 B, pin 356 B).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn identical_blocks_of_several_instructions_are_written_once() {
    let asm =
        driver::assembly(include_str!("data/identical_blocks_lru.ll"), Platform::Linux, Level::O3).expect("lowers");
    let zeroing =
        asm.split(".Lmain.0:").next().unwrap_or("").lines().filter(|line| line.contains("xzr, xzr, lsl")).count();
    assert_eq!(zeroing, 1, "{asm}");
}
