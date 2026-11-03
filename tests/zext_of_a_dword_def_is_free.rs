//! `isDef32` answered `false`, so `zext (add i32 %a, 1)` selected the
//! `MOV32rr` form of the zero-extension and left `movzx rax, esi` between
//! `inc esi` and the indexed load; the pin selects `SUBREG_TO_REG` of the
//! add itself and emits none.

use std::process::Command;

const IR: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
target triple = \"x86_64-unknown-linux-gnu\"\n\
define i64 @g(ptr %p, i32 %a) {\n\
  %b = add i32 %a, 1\n\
  %z = zext i32 %b to i64\n\
  %q = getelementptr i64, ptr %p, i64 %z\n\
  %v = load i64, ptr %q\n\
  ret i64 %v\n\
}\n";

#[test]
fn test_the_zero_extension_of_a_dword_result_is_no_instruction() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("g.ll");
    std::fs::write(&source, IR).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["--isel=dag", "-O2", "-S", "-o", "-"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("movzx"), "{text}");
}
