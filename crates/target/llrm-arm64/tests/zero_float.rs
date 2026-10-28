//! The float zero.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

/// 0.0 was `movz x9, #0` then `fmov d, x9`; clang makes it from the zero
/// register (`fmov d, xzr`, or `movi`).
#[test]
fn a_float_zero_is_made_from_the_zero_register() {
    let asm = driver::assembly(include_str!("data/zero_float.ll"), Platform::Darwin, true).expect("lowers");
    assert!(asm.contains("fmov d") && asm.contains("xzr"), "{asm}");
    assert!(asm.contains("fmov s") && asm.contains("wzr"), "{asm}");
    assert!(!asm.contains("movz"), "{asm}");
}
