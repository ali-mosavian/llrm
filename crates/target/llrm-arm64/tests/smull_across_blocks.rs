//! A sign extension made in one block and multiplied in a later one is a
//! `smull`.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

/// The live-out sign bits (`AssertSext`) were dropped before the matcher ran
/// (DEVIATE SD43), so `smullwithsignbits` saw a plain register: `sbfm x, x, #0,
/// #31` and a 64-bit `madd` where the pin has `smull x, w, w` (QB bench
/// `switch`: 74109 instructions against the pin's 68963).
#[test]
fn a_sign_extended_value_multiplied_in_a_later_block_is_a_smull() {
    let asm = driver::assembly(include_str!("data/smull_across_blocks.ll"), Platform::Linux, true).expect("lowers");
    assert_eq!(asm.matches("smaddl").count(), 3, "{asm}");
    assert!(!asm.contains("#0, #31"), "{asm}");
}
