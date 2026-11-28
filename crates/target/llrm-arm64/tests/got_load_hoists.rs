//! A GOT slot is constant memory (the pin: `PseudoSourceValue::GOT` is
//! constant, and `MachineLICM` hoists a load from it, MachineLICM.cpp
//! `mayLoadFromGOTOrConstantPool`). textfill -O2 at PIE kept `ldr x, [x,
//! :got_lo12:]` inside the loop that stores bytes through it: 9 instructions an
//! iteration where gcc and clang run 8 (+2000 on the run).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const LOOP: &str = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
@screen = external global [4000 x i8]
define void @f(i8 %v) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %n, %loop ]
  %b = trunc i64 %i to i8
  %w = add i8 %b, %v
  %p = getelementptr [4000 x i8], ptr @screen, i64 0, i64 %i
  store i8 %w, ptr %p
  %n = add i64 %i, 1
  %c = icmp eq i64 %n, 4000
  br i1 %c, label %done, label %loop
done:
  ret void
}
!llvm.module.flags = !{!0, !1}
!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
"#;

#[test]
fn a_got_load_is_made_once_outside_the_loop_that_stores_through_it() {
    let asm = driver::assembly(LOOP, Platform::Linux, Level::O2).expect("lowers");
    let body = asm.split_once(".Lf.1:").map(|(_, rest)| rest).unwrap_or_else(|| panic!("no loop label: {asm}"));
    let loop_got = body.lines().filter(|line| line.contains("got") || line.contains("screen")).count();
    assert_eq!(loop_got, 0, "{asm}");
    assert!(asm.lines().filter(|line| line.to_lowercase().contains("got")).count() >= 1, "{asm}");
}
