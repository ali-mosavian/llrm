//! Loads read by a sign extension.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn asm() -> String {
    driver::assembly(include_str!("data/sign_loads.ll"), Platform::Darwin, true).expect("the functions lower")
}

/// `sext (load i32)` was `ldr w` then `sbfm x, x, #0, #31`; clang makes one `ldrsw` (and `ldrsh`, `ldrsb`).
#[test]
fn a_load_that_is_only_sign_extended_is_a_sign_extending_load() {
    let asm = asm();
    for load in ["ldrsw x0, [x0]", "ldrsh x0, [x0]", "ldrsb w0, [x0]"] {
        assert!(asm.contains(load), "{load} missing from\n{asm}");
    }
    assert!(!asm.contains("sbfm x0, x0"), "{asm}");
}

/// A load also read as the narrow value is still one load: the extending one serves both readers.
#[test]
fn a_load_read_by_more_than_the_extension_is_left_alone() {
    let asm = asm();
    let body = &asm[asm.find("_also_read_narrow:").expect("the function")..];
    assert_eq!(body.matches("ldr").count(), 1, "{body}");
}
