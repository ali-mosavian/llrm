//! `llvm.is.constant` and `llvm.objectsize` at -O0, on the DAG path alone.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// No mid-end ran at -O0, so the intrinsic reached the selector, which refused it ("intrinsic call
/// llvm.is.constant.i32") and sent 26 runtime/qb programs and `tests/run/c/builtins_table.c` to the hand selector.
/// The pin lowers it before selection at every level.
#[test]
fn is_constant_is_lowered_before_the_dag_at_o0() {
    let ir = format!(
        "{LAYOUT}declare i1 @llvm.is.constant.i32(i32)\n\
define i32 @f(i32 %x) {{\n  %c = call i1 @llvm.is.constant.i32(i32 %x)\n  %r = zext i1 %c to i32\n  ret i32 %r\n}}\n"
    );
    let asm = driver::assembly(&ir, Platform::Linux, false).expect("the DAG path lowers llvm.is.constant at -O0");
    // A value that is not a constant at -O0: the intrinsic is 0.
    assert!(asm.contains("xzr") && !asm.contains("is.constant"), "{asm}");
}

/// The same for `llvm.objectsize`, which the pin answers "unknown" (-1 or 0) for a pointer it knows nothing of.
#[test]
fn objectsize_is_lowered_before_the_dag_at_o0() {
    let ir = format!(
        "{LAYOUT}declare i64 @llvm.objectsize.i64.p0(ptr, i1, i1, i1)\n\
define i64 @f(ptr %p) {{\n  %n = call i64 @llvm.objectsize.i64.p0(ptr %p, i1 false, i1 true, i1 false)\n  ret i64 %n\n}}\n"
    );
    driver::assembly(&ir, Platform::Linux, false).expect("the DAG path lowers llvm.objectsize at -O0");
}

/// `llvm.experimental.memset.pattern` reached the DAG at -O0 for the same reason, and the memset expansion
/// panicked ("a target with string instructions names their registers", memops.rs:2024): `memset_pattern` at
/// -O0 did not build. PreISelIntrinsicLowering runs at every level.
#[test]
fn memset_pattern_is_expanded_before_the_dag_at_o0() {
    let ir = format!(
        "{LAYOUT}declare void @llvm.experimental.memset.pattern.p0.i16.i64(ptr, i16, i64, i1)\n\
define void @fill(ptr %p, i64 %n) {{\n  call void @llvm.experimental.memset.pattern.p0.i16.i64(ptr %p, i16 1824, i64 %n, i1 false)\n  ret void\n}}\n"
    );
    driver::assembly(&ir, Platform::Linux, false).expect("the DAG path expands memset.pattern at -O0");
}
