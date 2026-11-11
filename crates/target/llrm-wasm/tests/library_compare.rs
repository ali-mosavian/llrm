//! `memcmp(p, q, n) == 0` stayed a `memcmp` call on wasm32: the target's C
//! library did not state `bcmp`, so `SimplifyLibCalls` never formed it.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

#[test]
fn a_memcmp_tested_for_equality_calls_bcmp() {
    let text = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
declare i32 @memcmp(ptr, ptr, i32)
define i1 @same(ptr %p, ptr %q, i32 %n) {
  %r = call i32 @memcmp(ptr %p, ptr %q, i32 %n)
  %c = icmp eq i32 %r, 0
  ret i1 %c
}
";
    let asm = assembly(text, Width::W32, Options { level: Level::O2, ..Options::default() }).expect("lowers");
    assert!(asm.contains("bcmp") && !asm.contains("memcmp"), "{asm}");
}
