//! At `optsize` every operand fold is worth its extra uses
//! (`shouldOptForSize` in `isWorthFoldingALU`). The zero extension of `%7`
//! feeds a `sub` and an address shift; only the shift took it, so the `sub` got
//! a separate `mov w9, w0`: recchop -Os was 4 B larger than the pin's.

use llrm_arm64::driver::{self, IselMode, Level};
use llrm_arm64::machine::Platform;

const SOURCE: &str = r#"target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
@values = internal global [64 x i32] zeroinitializer
define i64 @chop(i32 %0, i32 %1) optsize {
  %3 = add nsw i32 %1, -2
  %4 = icmp sgt i32 %0, %3
  br i1 %4, label %33, label %5

5:
  %wide.trip.count = zext i32 %1 to i64
  br label %6

6:
  %7 = phi i32 [ %10, %27 ], [ %0, %5 ]
  %8 = phi i64 [ %31, %27 ], [ 0, %5 ]
  %9 = add nuw nsw i32 %7, %1
  %10 = lshr i32 %9, 1
  %11 = icmp samesign ult i32 %7, %1
  br i1 %11, label %12, label %27

12:
  %13 = zext nneg i32 %7 to i64
  %14 = sub i64 %wide.trip.count, %13
  %15 = shl nuw nsw i64 %13, 2
  %16 = getelementptr i8, ptr @values, i64 %15
  br label %17

17:
  %18 = phi ptr [ %26, %17 ], [ %16, %12 ]
  %19 = phi i64 [ %25, %17 ], [ %14, %12 ]
  %20 = phi i64 [ %24, %17 ], [ 0, %12 ]
  %21 = load i32, ptr %18, align 4
  %22 = and i32 %21, %10
  %23 = zext nneg i32 %22 to i64
  %24 = add nuw nsw i64 %20, %23
  %25 = add i64 %19, -1
  %26 = getelementptr i8, ptr %18, i64 4
  %exitcond = icmp eq i64 %25, 0
  br i1 %exitcond, label %27, label %17

27:
  %28 = phi i64 [ 0, %6 ], [ %24, %17 ]
  %29 = call i64 @chop(i32 %7, i32 %10)
  %30 = add i64 %28, %8
  %31 = add i64 %30, %29
  %32 = icmp sgt i32 %10, %3
  br i1 %32, label %33, label %6

33:
  %accumulator.tr.lcssa = phi i64 [ 0, %2 ], [ %31, %27 ]
  ret i64 %accumulator.tr.lcssa
}
"#;

/// Does some `sub` take its second operand as `wN, uxtw`? At -O0, where LSR
/// leaves the shape alone.
fn folds_the_extend(source: &str) -> bool {
    driver::set_isel_mode(IselMode::Dag);
    let asm = driver::assembly(source, Platform::Linux, Level::O0).expect("lowers");
    asm.lines().map(str::trim).any(|line| line.starts_with("sub ") && line.contains(", uxtw"))
}

#[test]
fn a_shared_zero_extend_folds_into_a_sub_at_optsize() {
    assert!(
        folds_the_extend(SOURCE),
        "no `sub xD, xN, wM, uxtw`:\n{}",
        driver::assembly(SOURCE, Platform::Linux, Level::O0).unwrap()
    );
}
