//! -O0 multiplies by a constant with `imul`, as the pin's -O0 does.

use std::process::Command;

fn asm(
    level: &str,
    scratch: &std::path::Path,
) -> String {
    // 10000 is no `3|5|9 << n` of the pin's `combineMul`: it keeps `imul`.
    std::fs::write(scratch.join("a.c"), "int f(int a) { return a * 10000; }\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .current_dir(scratch)
        .args([level, "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    std::fs::read_to_string(scratch.join("a.s")).unwrap()
}

/// The 486 clock sheet priced the constant multiply at -O0 too, so `a * 10000`
/// became a chain of `lea` and `shl` (13 steps for the 64-bit multiply of
/// wide.c: 514 B against the pin's 353 B). -O0 has no speed goal.
#[test]
fn o0_multiplies_by_a_constant_with_imul() {
    let scratch = tempfile::tempdir().unwrap();
    let text = asm("-O0", scratch.path());
    assert!(text.contains("imul"), "-O0 did not use imul:\n{text}");
}
