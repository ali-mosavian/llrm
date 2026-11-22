//! matmul -O3 (arm64): the pin fully unrolls the checksum nest, outer loop
//! too; ours kept the outer loop rolled because the byte price of a full unroll
//! (500 mclk/B, DEVIATE UP1) vetoed it: 212 clk saved for 760 B. -O3 is judged
//! by clocks, so the pin's threshold alone decides there (UP1-O3): 603 -> 566
//! modelled clocks on matmul. -O2 keeps the price.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn back_edges(level: Level) -> usize {
    let asm = driver::assembly(include_str!("data/unroll_o3_outer_nest.ll"), Platform::Linux, level).expect("lowers");
    asm.lines().filter(|line| ["\tb.", "\tcbnz", "\tcbz", "\ttbnz"].iter().any(|m| line.starts_with(m))).count()
}

#[test]
fn an_8x8_nest_the_pin_unrolls_is_fully_unrolled_at_o3() {
    assert_eq!(back_edges(Level::O3), 0);
}

#[test]
fn the_byte_price_still_keeps_the_outer_loop_rolled_at_o2() {
    assert!(back_edges(Level::O2) >= 1);
}
