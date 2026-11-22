//! Fields at offsets an `ldr` cannot reach are reached from one base.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const FIELDS: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
target triple = \"aarch64-unknown-linux-gnu\"
define void @f(ptr %s, i32 %v) {
  %t = trunc i32 %v to i16
  %a = getelementptr i8, ptr %s, i64 11936
  store i16 %t, ptr %a, align 2
  %b = getelementptr i8, ptr %s, i64 12938
  %u = trunc i32 %v to i8
  store i8 %u, ptr %b, align 1
  %c = getelementptr i8, ptr %s, i64 12944
  store i16 %t, ptr %c, align 2
  ret void
}
";

/// QuickrBasic `dbug_eval` -Os: each field at 0x2ea0 and beyond was a `mov` of
/// its offset and a register-offset `strh`/`strb`: three `mov`s, no shared base
/// (+100 B in the function against the pin, which adds 11936 once and stores
/// at `[x, #1002]` and `[x, #1008]`).
#[test]
fn fields_past_the_immediate_reach_share_one_base() {
    let asm = driver::assembly(FIELDS, Platform::Linux, Level::Os).expect("lowers");
    let moves = asm.lines().filter(|line| line.trim().starts_with("movz w")).count();
    let adds = asm.lines().filter(|line| line.trim().starts_with("add x")).count();
    assert!(moves == 1 && adds == 1, "{moves} offset moves, {adds} base adds:\n{asm}");
    assert!(asm.contains("#1002]") && asm.contains("#1008]"), "{asm}");
}
