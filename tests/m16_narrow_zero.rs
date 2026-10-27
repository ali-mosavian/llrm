//! A zero used only as a byte or a word is made at that width at m16.

use std::process::Command;

const KERNEL: &str = "DEFINT A-Z
DIM i AS INTEGER
INPUT i
OUT 969, 0
OUT 969, i
";

/// `OUT 969, 0` was `xor eax, eax` (66 31 C0, 3 B) where the old selector had
/// `mov al, 0` (2 B): the pin's `(i8 0)` is `EXTRACT_SUBREG (MOV32r0)`, and the
/// 66 prefix is real mode's. deedlines under the DAG selector: 92 such zeros.
#[test]
fn test_a_byte_zero_is_not_a_dword_xor() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.bas"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["--isel=dag", "a.bas", "--dialect", "qb45", "--runtime", "qb45", "-Os", "-S", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    assert!(!asm.contains("xor eax, eax") && !asm.contains("xor ebx, ebx"), "{asm}");
}
