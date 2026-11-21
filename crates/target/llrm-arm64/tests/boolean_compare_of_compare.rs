//! `(x != C) == 0` is one compare, however the booleans are typed.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const IR: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
target triple = \"aarch64-unknown-linux-gnu\"
declare void @g()
define void @f(i32 %x) optsize {
  %c = icmp ne i32 %x, 5064018
  %z = zext i1 %c to i32
  %e = icmp eq i32 %z, 0
  br i1 %e, label %t, label %n
t:
  call void @g()
  ret void
n:
  ret void
}
";

/// QuickrBasic `indt_scan` -Os: the compare of a zero-extended compare with 0
/// was legalized to `i32` over `i32` and the inner compare stayed: `cmp;
/// csinc; cbz` for the pin's `cmp; b.eq` (+44 B in the function).
#[test]
fn a_compare_of_a_zero_extended_compare_with_zero_is_the_inverse_compare() {
    let asm = driver::assembly(IR, Platform::Linux, Level::O0).expect("lowers");
    assert!(!asm.contains("csinc") && !asm.contains("cbz"), "the compare is materialized:\n{asm}");
}
