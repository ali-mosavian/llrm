//! A function returning an undefined value computes nothing.

use std::process::Command;

const IR: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\
target triple = \"i386-unknown-unknown-code16\"\n\
define i64 @f() addrspace(1) {\n  ret i64 undef\n}\n";

/// `ret i64 undef` (a body that falls off the end) was `xor edx,edx; xor
/// eax,eax` under the DAG selector, where the pin emits nothing: the
/// `IMPLICIT_DEF` was materialised as zero (+4 B in value_of_valueless).
#[test]
fn test_returning_undef_emits_no_instruction() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.ll"), IR).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .current_dir(scratch.path())
        .args(["-Os", "-mabi=cdecl", "--isel=dag", "-S", "-o", "-", "a.ll"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    let body = asm.split("_f proc far").nth(1).unwrap().split("_f endp").next().unwrap();
    assert!(!body.contains("xor") && !body.contains("mov"), "{asm}");
}
