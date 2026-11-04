//! A branch on whether a call's result is zero tests the result register.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/call_result_branch.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// `bl g; mov w9, w0; cbz w9` was a copy of the result to clear bits `cbz w`
/// does not read: clang tests `w0`.
#[test]
fn an_int_result_is_tested_in_w0() {
    let text = body("int");
    assert!(text.contains("cbz w0") || text.contains("cbnz w0"), "{text}");
    assert!(!text.contains("mov w") && !text.contains("orr w"), "{text}");
}

#[test]
fn a_long_result_is_tested_in_x0() {
    let text = body("long");
    assert!(text.contains("cbz x0") || text.contains("cbnz x0"), "{text}");
}

#[test]
fn a_byte_result_is_cleared_before_the_test() {
    let text = body("byte");
    assert!(text.contains("and w") || text.contains("uxtb"), "{text}");
}
