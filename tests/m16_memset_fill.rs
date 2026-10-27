//! A constant memset of a local, tuned for size, is a fill when its stores
//! are longer.

use std::process::Command;

const ARRAY: &str = "DEFINT A-Z
DECLARE SUB f ()
f
SUB f
DIM pal(256, 3)
pal(1, 1) = INP(&H3C9)
PRINT pal(1, 1)
END SUB
";

/// The 22-byte descriptor of a local array was zeroed by five `mov dword ptr
/// [bp-n], eax` and a `mov word ptr`, 28 bytes, where `rep stosb` is 12
/// (qbdemo WHITEFADE and UNWHITEFADE +16 B each under the DAG selector).
#[test]
fn test_a_local_array_descriptor_is_zeroed_by_a_fill() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.bas"), ARRAY).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["--isel=dag", "a.bas", "--dialect", "qb45", "--runtime", "qb45", "-Os", "-S", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    assert!(asm.contains("rep stosb") && !asm.contains("dword ptr [bp-10], eax"), "{asm}");
}
