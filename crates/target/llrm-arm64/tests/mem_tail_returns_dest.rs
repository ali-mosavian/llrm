//! A `memcpy` whose destination the function returns is a tail call.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const GROW: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
target triple = \"aarch64-unknown-linux-gnu\"
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare ptr @alloc(i64)
define ptr @f(ptr %src, i64 %n) {
  %p = tail call ptr @alloc(i64 %n)
  tail call void @llvm.memcpy.p0.p0.i64(ptr %p, ptr %src, i64 %n, i1 false)
  ret ptr %p
}
";

/// QuickrBasic `arena_grow` -Os: `ret p` after `memcpy(p, ..)` kept `p` in a
/// callee-saved register and called `memcpy` (+16 B against the pin, which
/// jumps to it: the routine returns its destination).
#[test]
fn a_memcpy_whose_destination_is_returned_is_a_tail_call() {
    let asm = driver::assembly(GROW, Platform::Linux, Level::Os).expect("lowers");
    assert!(asm.lines().any(|line| line.trim() == "b memcpy"), "no jump to memcpy:\n{asm}");
    assert!(!asm.contains("bl memcpy"), "memcpy is called:\n{asm}");
}
