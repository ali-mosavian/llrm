//! Float constants the selector stores.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

/// A float constant `fmov` cannot make went through a float register
/// (`movz`..`fmov d16, x9`, `str d16`); clang stores the integer of its bits
/// (`str x9`), and `str xzr` for zero (tests/llc/fconst).
#[test]
fn a_float_constant_is_stored_as_integer_bits() {
    let text = include_str!("data/float_constant_store.ll");
    let asm = driver::assembly(text, Platform::Darwin, true).expect("the functions lower");
    assert!(!asm.contains("fmov"), "{asm}");
    assert!(asm.lines().any(|line| line.trim().starts_with("str xzr")), "{asm}");
}
