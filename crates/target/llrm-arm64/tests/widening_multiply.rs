//! The product of two 32-bit values extended to 64 bits.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/widening_multiply.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("function");
    let rest = &asm[start + 1..];
    asm[start..start + 1 + rest.find("\n_").unwrap_or(rest.len())].to_owned()
}

/// `acc + (long)(a * b)`: the pin's `llc` makes the 32-bit product and adds it
/// sign-extended (`mul w8, w1, w2; add x0, x0, w8, sxtw`): the extension is
/// of the wrapped product, which `smaddl` of the operands does not compute.
#[test]
fn a_widened_product_added_is_a_multiply_and_an_extended_add() {
    let asm = body("accumulate");
    assert!(asm.contains("madd w") && asm.contains("sxtw") && !asm.contains("smaddl"), "{asm}");
    assert!(!asm.contains("sbfm"), "{asm}");
    let subtract = body("subtract");
    assert!(subtract.contains("sub x0, x0, w") && subtract.contains("sxtw"), "{subtract}");
}

/// Alone it is the product and a `sxtw` (`sbfm x, x, #0, #31`).
#[test]
fn a_widened_product_alone_is_a_multiply_and_a_sign_extension() {
    let asm = body("alone");
    assert!(asm.contains("madd w") && asm.contains("sbfm x0, x9, #0, #31"), "{asm}");
}

/// Without `nsw` the 32-bit product may wrap before it is extended, so the
/// 64-bit product is another value.
#[test]
fn a_product_that_may_wrap_is_not_widened() {
    let asm = body("may_wrap");
    assert!(!asm.contains("smaddl") && asm.contains("madd w"), "{asm}");
}
