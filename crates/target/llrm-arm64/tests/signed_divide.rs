//! Signed division by a constant.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/signed_divide.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// `x / 7` took the product's high half with its own `ubfm` and then added the dividend: seven instructions
/// after the constant; clang adds the dividend to the product shifted (`add x8, x0, x8, lsr #32`).
#[test]
fn the_high_half_of_the_product_is_a_shifted_operand_of_the_adjusting_add() {
    let asm = body("by_seven");
    assert!(asm.contains("lsr #32"), "{asm}");
    assert!(!asm.contains("ubfm"), "{asm}");
}

/// Without an adjustment the quotient's shift and the high half are one `asr x, #32+shift`.
#[test]
fn the_quotient_shift_and_the_high_half_are_one_shift() {
    let asm = body("by_five");
    assert_eq!(asm.matches("sbfm").count() + asm.matches("ubfm").count(), 1, "{asm}");
}
