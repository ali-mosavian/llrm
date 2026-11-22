//! A global's address and the page of a later folded store are separate values.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// QuickrBasic uimain `uimn_act` -Os (+116 B): the page of `@g + 8`, read by
/// the `add :lo12:` that made the address and by a store after the calls, was
/// one value live across them, so a seventh callee-saved register held the
/// page beside the address. The pin's `MOVaddr` is the whole address: the
/// store makes its own page where it is, in a caller-saved register.
#[test]
fn the_page_of_a_store_after_calls_is_not_held_beside_the_address() {
    let asm =
        driver::assembly(include_str!("data/address_page_per_reader.ll"), Platform::Linux, Level::Os).expect("lowers");
    let folded: Vec<&str> = asm.lines().filter(|line| line.contains("g@PAGEOFF+8]")).collect();
    assert_eq!(folded.len(), 1, "one store folds the page offset: {asm}");
    let base: u8 = folded[0]
        .split('[')
        .nth(1)
        .and_then(|rest| rest[1..].split(',').next())
        .and_then(|n| n.parse().ok())
        .expect("a register");
    assert!(base < 19, "the page of the store after the calls sits in x{base}, a callee-saved register:\n{asm}");
}
