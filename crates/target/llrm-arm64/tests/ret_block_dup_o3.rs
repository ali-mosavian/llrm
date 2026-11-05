//! At -O3 layout copies a block of up to 4 instructions, so `fib`'s base case
//! returns on its own: the pin's `add w0, w0, wzr; ldp; ldp; ret` beside the
//! loop's exit (-O3: 3 instructions per call over the pin, +4.3%). Below -O3
//! the limit is 2 and the exits share one epilogue.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn epilogues(level: Level) -> usize {
    let asm = driver::assembly(include_str!("data/ret_block_dup_o3.ll"), Platform::Linux, level).expect("lowers");
    let fib = asm.split("\nfib:").nth(1).expect("fib").split("\nmain:").next().expect("a body");
    fib.matches("ldp x29, x30, [sp], #").count()
}

#[test]
fn a_return_block_of_four_is_copied_at_o3_and_not_below() {
    assert_eq!((epilogues(Level::O3), epilogues(Level::O2)), (2, 1));
}
