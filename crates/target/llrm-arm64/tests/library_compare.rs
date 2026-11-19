//! `memcmp(p, q, n) == 0` stayed a `memcmp` call on arm64: the target's C
//! library did not state `bcmp`, so `SimplifyLibCalls` never formed it.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

#[test]
fn a_memcmp_tested_for_equality_calls_bcmp() {
    let text = "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"
target triple = \"arm64-apple-macosx10.4.0\"
declare i32 @memcmp(ptr, ptr, i64)
define i1 @same(ptr %p, ptr %q, i64 %n) {
  %r = call i32 @memcmp(ptr %p, ptr %q, i64 %n)
  %c = icmp eq i32 %r, 0
  ret i1 %c
}
";
    let asm = driver::assembly(text, Platform::Darwin, true).expect("lowers");
    assert!(asm.contains("bcmp") && !asm.contains("memcmp"), "{asm}");
}
