//! A dword constant argument is pushed as the smaller of `pushd` and two
//! `pushw` at m16 -Os.

use std::process::Command;

/// `HEX$(-1&)` pushed `pushd 65535` (66 68 imm32, 6 B) where the old selector
/// pushes `pushw 0; pushw 65535` (5 B): hex_oct_negative.bas grew 29 B.
#[test]
fn test_a_dword_constant_with_a_byte_half_is_two_word_pushes() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.bas"), "PRINT HEX$(-1%); HEX$(255%)\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["--isel=dag", "a.bas", "--dialect", "qb45", "--runtime", "qb45", "-Os", "-S", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    assert!(!asm.contains("pushd 65535") && asm.contains("pushw 65535"), "{asm}");
}
