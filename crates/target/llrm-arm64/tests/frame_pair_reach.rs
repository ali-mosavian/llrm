//! A frame larger than a pre-indexed allocation keeps the pin's object order
//! (the first object at the top): the dense order put a 512-byte array past
//! the 504-byte reach of `ldp`, so two adjacent loads stayed two (matmul -O3:
//! 10 pairs unpaired, +52 bytes over the pin).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn two_adjacent_loads_in_a_large_frame_still_pair() {
    let asm = driver::assembly(include_str!("data/frame_pair_reach.ll"), Platform::Linux, Level::O2).expect("lowers");
    let pairs = asm.lines().filter(|line| line.contains("ldp") && !line.contains("x29")).count();
    assert_eq!(pairs, 1, "{asm}");
}
