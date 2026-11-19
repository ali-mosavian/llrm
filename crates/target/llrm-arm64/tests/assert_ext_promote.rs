//! A promoted `AssertSext` (`PromoteIntRes_AssertSext`): `@bench_qlight` named
//! `Unsupported { op: AssertSext, ty: I16, what: "promote result" }` and fell
//! back to the hand selector.

use std::process::Command;

const IR: &str = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@input_raw = global i16 200, align 2
@input_s1 = global i16 120, align 2
@input_s2 = global i16 60, align 2
@input_s3 = global i16 240, align 2
@input_low = global i16 -20, align 2

define i16 @ls_scale_byte(i16 %0, i16 %1) {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i64, align 8
  store i16 %0, ptr %3, align 2
  store i16 %1, ptr %4, align 2
  call void @llvm.lifetime.start.p0(ptr %5)
  %6 = load i16, ptr %3, align 2
  %7 = sext i16 %6 to i64
  %8 = load i16, ptr %4, align 2
  %9 = sext i16 %8 to i64
  %10 = mul nsw i64 %7, %9
  %11 = sdiv i64 %10, 120
  store i64 %11, ptr %5, align 8
  %12 = load i64, ptr %5, align 8
  %13 = icmp sgt i64 %12, 255
  br i1 %13, label %14, label %15

14:                                               ; preds = %2
  store i64 255, ptr %5, align 8
  br label %15

15:                                               ; preds = %14, %2
  %16 = load i64, ptr %5, align 8
  %17 = icmp slt i64 %16, 0
  br i1 %17, label %18, label %19

18:                                               ; preds = %15
  store i64 0, ptr %5, align 8
  br label %19

19:                                               ; preds = %18, %15
  %20 = load i64, ptr %5, align 8
  %21 = trunc i64 %20 to i16
  call void @llvm.lifetime.end.p0(ptr %5)
  ret i16 %21
}

declare void @llvm.lifetime.start.p0(ptr captures(none))

declare void @llvm.lifetime.end.p0(ptr captures(none))

define i64 @bench_qlight() {
  %1 = load volatile i16, ptr @input_raw, align 2
  %2 = load volatile i16, ptr @input_s1, align 2
  %3 = call i16 @ls_scale_byte(i16 %1, i16 %2)
  %4 = sext i16 %3 to i64
  %5 = mul nsw i64 %4, 1000000
  %6 = load volatile i16, ptr @input_raw, align 2
  %7 = load volatile i16, ptr @input_s2, align 2
  %8 = call i16 @ls_scale_byte(i16 %6, i16 %7)
  %9 = sext i16 %8 to i64
  %10 = mul nsw i64 %9, 1000
  %11 = add nsw i64 %5, %10
  %12 = load volatile i16, ptr @input_raw, align 2
  %13 = load volatile i16, ptr @input_s3, align 2
  %14 = call i16 @ls_scale_byte(i16 %12, i16 %13)
  %15 = sext i16 %14 to i64
  %16 = add nsw i64 %11, %15
  %17 = load volatile i16, ptr @input_low, align 2
  %18 = load volatile i16, ptr @input_s1, align 2
  %19 = call i16 @ls_scale_byte(i16 %17, i16 %18)
  %20 = sext i16 %19 to i64
  %21 = add nsw i64 %16, %20
  ret i64 %21
}

"#;

#[test]
fn a_sign_asserted_halfword_result_is_promoted_not_refused() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, IR).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-O2", "-S", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    let why = String::from_utf8_lossy(&said.stderr);
    assert!(said.status.success(), "{why}");
    assert!(!why.contains("AssertSext"), "the DAG path refused the assertion: {why}");
}
