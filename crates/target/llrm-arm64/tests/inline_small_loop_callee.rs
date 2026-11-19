//! AArch64 inlines as the pin does: a callee at `cost=-5` against the pin's
//! threshold of 250 goes in. The 62.5 milliclock a byte rate (the 11-04 rule
//! for small platforms) refused `fillw` and `filll` of pattern_fill (`main ->
//! fillw: cost=-5 threshold=-5 no`): -O3 1,840 instructions, clang 602.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_small_loop_callee_at_a_hot_site_is_inlined_at_o2() {
    let asm =
        driver::assembly(include_str!("data/inline_small_loop_callee.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert!(!asm.contains("bl fillw"), "{asm}");
}
