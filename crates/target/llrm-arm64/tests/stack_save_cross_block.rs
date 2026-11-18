//! A `stacksave` read in one block and restored in another was read again at
//! the restore: the allocator saw `add xN, sp, #0` as a constant (the read of
//! `sp` was no operand) and made it again after the `alloca` had lowered `sp`,
//! so `stackrestore` freed nothing and each trip's array sat 16 B lower
//! (a VLA in a loop, -Os: `-16 -32 -48 -64 -96` for `0 0 0 0 -16`).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_stack_save_is_read_before_the_alloca_that_follows_it() {
    let asm =
        driver::assembly(include_str!("data/stack_save_cross_block.ll"), Platform::Linux, Level::Os).expect("lowers");
    let body: Vec<&str> = asm.lines().skip_while(|line| *line != "f:").collect();
    let lowered = body.iter().position(|line| line.starts_with("\tadd sp, ")).expect("the alloca writes sp");
    let reads = body[..lowered].iter().filter(|line| line.contains(", sp, #0") && !line.contains("x29")).count();
    assert_eq!(reads, 2, "the alloca's read and the save precede the write:\n{}", body.join("\n"));
}
