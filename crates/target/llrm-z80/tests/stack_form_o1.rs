//! The push and pop form of a function is kept at -O1 by the same rule as the
//! other variants.

use llrm_z80::driver::{Level, Request};
use llrm_z80::expand::price_emitted;

/// loop_fusion -O1 `same` kept `n` in a frame slot (`ld c, -2(ix)`, 3 B a
/// reload) at 124 B, because the push and pop form, 111 B for 4% more weighted
/// clocks, was judged at 1 B = 3 T. -O1 is strict on both axes, so the smaller
/// relative regret wins; the cell was 923 B against SDCC's 901 B.
#[test]
fn a_function_at_o1_keeps_the_stack_form_that_is_far_smaller_for_few_clocks() {
    let ir = include_str!("data/loop_fusion_o1.ll");
    let lines = llrm_z80::compile::lines(ir, "same", 0, &Request::new(Level::O1)).unwrap_or_else(|why| panic!("{why}"));
    let (bytes, clocks) = price_emitted(&lines, true);
    assert!(bytes <= 111, "same is {bytes} B / {clocks} T at -O1, the framed form is 124 B");
}
