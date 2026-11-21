//! Two compares joined by `&&` become one `cmp` and a `ccmp` at `-Os`.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const BOTH: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
target triple = \"aarch64-unknown-linux-gnu\"
declare void @g()
define void @f(ptr %p, i32 %k) optsize {
  %a = load i8, ptr %p, align 1
  %c1 = icmp ugt i8 %a, 63
  %c2 = icmp eq i32 %k, 129
  %c = select i1 %c1, i1 %c2, i1 false
  br i1 %c, label %t, label %e
t:
  call void @g()
  ret void
e:
  ret void
}
";

/// QuickrBasic `tty_parse` and `args` -Os: the optsize rule that keeps two
/// compares as two branches (m16's, SD103) applied to arm64 too, where the
/// pin's cost rule decides: two `cmp`s and two branches, no `ccmp` (+48 B and
/// +64 B against the pin).
#[test]
fn two_compares_joined_by_and_are_one_ccmp_at_optsize() {
    let asm = driver::assembly(BOTH, Platform::Linux, Level::Os).expect("lowers");
    assert!(asm.contains("ccmp "), "no ccmp:\n{asm}");
}
