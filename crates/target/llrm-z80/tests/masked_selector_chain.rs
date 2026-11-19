//! A value masked once and compared in later blocks.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `s = w & 7` compared with 3, 2 and 1 in three blocks was masked again at
/// each compare (`and a, #7` four times, 2 B each): the compare was narrowed
/// to three bits and the promoted operand re-masked, beside the mask kept for
/// the later blocks.
#[test]
fn a_selector_masked_once_is_not_masked_again_at_each_compare() {
    let asm = text(include_str!("data/masked_selector_chain.ll"), "f", 0, &Request::new(Level::Os)).expect("compiles");
    let masks = asm.lines().filter(|line| line.trim().starts_with("and a, #")).count();
    assert!(masks <= 1, "{masks} masks:\n{asm}");
}
