//! The pre-RA machine scheduler runs on arm64
//! (`Target::schedules_before_allocation`). Unordered, a loop-carried counter
//! was written to a fresh register before the compare read the old one and
//! copied back (`add w3, w7, #1; ...; cmp w7, #31; orr x7, xzr, x3`): one more
//! instruction per trip and one more live register. `mandel` at every level
//! lost 8873 executed instructions to it.

use std::process::Command;

const MANDEL: &str = r#"target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define i64 @f() {
  br label %1
1:
  %2 = phi i64 [ -12, %0 ], [ %35, %34 ]
  %3 = phi i64 [ 0, %0 ], [ %31, %34 ]
  %4 = mul nsw i64 %2, 24
  br label %5
5:
  %6 = phi i64 [ -16, %1 ], [ %32, %28 ]
  %7 = phi i64 [ %3, %1 ], [ %31, %28 ]
  %8 = mul nsw i64 %6, 24
  %9 = add nsw i64 %8, -128
  br label %10
10:
  %11 = phi i64 [ 0, %5 ], [ %23, %20 ]
  %12 = phi i64 [ 0, %5 ], [ %25, %20 ]
  %13 = phi i16 [ 0, %5 ], [ %26, %20 ]
  %14 = mul nsw i64 %12, %12
  %15 = lshr i64 %14, 8
  %16 = mul nsw i64 %11, %11
  %17 = lshr i64 %16, 8
  %18 = add nuw nsw i64 %17, %15
  %19 = icmp samesign ugt i64 %18, 1024
  br i1 %19, label %28, label %20
20:
  %21 = mul nsw i64 %11, %12
  %22 = ashr i64 %21, 7
  %23 = add nsw i64 %22, %4
  %24 = add nsw i64 %9, %15
  %25 = sub nsw i64 %24, %17
  %26 = add nuw nsw i16 %13, 1
  %27 = icmp samesign ult i16 %13, 31
  br i1 %27, label %10, label %28
28:
  %29 = phi i16 [ %13, %10 ], [ 32, %20 ]
  %30 = zext nneg i16 %29 to i64
  %31 = add nsw i64 %7, %30
  %32 = add nsw i64 %6, 1
  %33 = icmp eq i64 %32, 16
  br i1 %33, label %34, label %5
34:
  %35 = add nsw i64 %2, 1
  %36 = icmp eq i64 %35, 12
  br i1 %36, label %37, label %1
37:
  ret i64 %31
}
"#;

fn assembly(
    env: &[(&str, &str)],
    text: &str,
) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, text).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .envs(env.iter().copied())
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

fn copies(text: &str) -> usize {
    text.lines().filter(|line| line.trim_start().starts_with("orr x") && line.contains(", xzr, x")).count()
}

#[test]
fn a_loop_counter_is_updated_in_place_not_through_a_copy() {
    let ordered = assembly(&[], MANDEL);
    let unordered = assembly(&[("LLRM_NO_PRESCHED", "1")], MANDEL);
    assert!(copies(&ordered) < copies(&unordered), "ordered:\n{ordered}\nunordered:\n{unordered}");
}
