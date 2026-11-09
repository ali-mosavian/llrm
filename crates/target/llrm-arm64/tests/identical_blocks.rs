//! Identical blocks that are more than a jump merge (`BranchFolder`): lru -O3
//! wrote `mov x12, xzr; mov x13, x12; b L` four times (368 B, pin 356 B).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// Since #7394 the zero is made twice (`orr x14, xzr, xzr; orr x8, xzr, xzr`)
/// where it was made once and copied (`mov x12, xzr; mov x13, x12`): the
/// shared block has two zeroing lines, so the test counts runs of them, one
/// per written block. Four unmerged copies would be four runs.
#[test]
fn identical_blocks_of_several_instructions_are_written_once() {
    let asm =
        driver::assembly(include_str!("data/identical_blocks_lru.ll"), Platform::Linux, Level::O3).expect("lowers");
    let before_main = asm.split(".Lmain.0:").next().unwrap_or("");
    let (mut runs, mut inside) = (0, false);
    for line in before_main.lines() {
        let zeroing = line.contains("xzr, xzr, lsl");
        runs += usize::from(zeroing && !inside);
        inside = zeroing;
    }
    assert_eq!(runs, 1, "{asm}");
}
