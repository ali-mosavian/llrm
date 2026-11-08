//! A narrow counter whose chain is masked and extended is widened.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn instructions(source: &str) -> usize {
    let asm = driver::assembly(source, Platform::Linux, Level::O2).expect("lowers");
    asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count()
}

/// ring -O2: `(i*5+3)&1023` on a `short i` stayed an i16 chain LSR does not
/// see: the multiply and add ran every trip, 52,105 instructions to gcc's
/// 41,128. Widened and reduced: 26 instructions, not 28.
#[test]
fn a_short_counter_masked_into_an_index_is_widened_and_reduced() {
    let insns = instructions(include_str!("data/bench_ring.ll"));
    assert!(insns < 28, "{insns} instructions");
}
