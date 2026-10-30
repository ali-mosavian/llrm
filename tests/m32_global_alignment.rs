//! A global states its alignment in the IR; the object keeps it.

mod common;

use std::process::Command;

/// `llrm-llc` placed `align 8` globals right after a byte, with no
/// alignment: x_matmul_f's `fmul qword` read 3744 misaligned doubles, +16%
/// clocks.
#[test]
fn test_a_global_double_array_is_aligned_to_its_stated_eight() {
    let ir = "target datalayout = \"@LAYOUT@\"\n\
              @C = internal global i8 1, align 1\n\
              @A = internal global [4 x double] zeroinitializer, align 8\n\
              define ptr @f() { ret ptr @A }\n\
              define ptr @g() { ret ptr @C }\n"
        .replace("@LAYOUT@", &common::datalayout("x86-m32"));
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.ll"), ir).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O1", "-S", "-o", "-", "a.ll"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    assert!(asm.contains("align 8\n_A label byte"), "{asm}");
}
