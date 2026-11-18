//! A multiply of post-indexed extending loads is `smaddl`: ComputeNumSignBits
//! looks through `LoadPost` as the pin's `case ISD::LOAD` does. dot -O1 ran
//! 10,426 modelled clocks against the pin's 8,526: the loop's loads were
//! indexed, their sign bits unknown, and `madd` (4 clocks) took the accumulate
//! chain from `smaddl` (3).

use std::process::Command;

const IR: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
target triple = \"aarch64-unknown-linux-gnu\"
define { i64, ptr } @f(ptr %a, ptr %b, i64 %acc) {
  %x = load i16, ptr %a
  %y = load i16, ptr %b
  %sx = sext i16 %x to i64
  %sy = sext i16 %y to i64
  %m = mul nsw i64 %sx, %sy
  %r = add nsw i64 %m, %acc
  %pa = getelementptr i16, ptr %a, i64 1
  %v = insertvalue { i64, ptr } undef, i64 %r, 0
  %w = insertvalue { i64, ptr } %v, ptr %pa, 1
  ret { i64, ptr } %w
}
";

#[test]
fn a_multiply_of_a_post_indexed_sign_extending_load_is_smaddl() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, IR).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O1", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(asm.contains("], #2"), "premise: the load is post-indexed:\n{asm}");
    assert!(asm.contains("smaddl") && !asm.contains("madd x"), "{asm}");
}
