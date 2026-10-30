//! The product of two 32-bit values extended to 64 bits.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/widening_multiply.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("function");
    let rest = &asm[start + 1..];
    asm[start..start + 1 + rest.find("\n_").unwrap_or(rest.len())].to_owned()
}

/// `acc + (long)(a * b)` was `madd w`, `sbfm`, `add`: three instructions where
/// `smaddl` is one.
#[test]
fn a_widened_product_added_is_smaddl() {
    let asm = body("accumulate");
    assert!(asm.contains("smaddl x"), "{asm}");
    assert!(!asm.contains("sbfm") && !asm.contains("madd w"), "{asm}");
    assert!(body("subtract").contains("smsubl x"));
    assert!(body("unsigned_accumulate").contains("umaddl x"));
}

/// Alone it is `smull` (`smaddl` from the zero register).
#[test]
fn a_widened_product_alone_is_smull() {
    let asm = body("alone");
    assert!(asm.contains("smaddl x") && asm.contains("xzr"), "{asm}");
}

/// Without `nsw` the 32-bit product may wrap before it is extended, so the
/// 64-bit product is another value.
#[test]
fn a_product_that_may_wrap_is_not_widened() {
    let asm = body("may_wrap");
    assert!(!asm.contains("smaddl") && asm.contains("madd w"), "{asm}");
}
