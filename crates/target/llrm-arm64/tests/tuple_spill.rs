//! A tuple held across a call: every vector register is destroyed by it, so
//! the allocator may not keep the `ld4` result in `v0`..`v3` over the call.
//! It must spill the four members and reload them (`str q`/`ldr q` each).
//! Matching the call's register mask against the tuple's own root, which no
//! call names, left it in registers: the call then overwrote the data.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const IR: &str = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare {<4 x i32>, <4 x i32>, <4 x i32>, <4 x i32>} @llvm.aarch64.neon.ld4.v4i32.p0(ptr)
declare void @llvm.aarch64.neon.st4.v4i32.p0(<4 x i32>, <4 x i32>, <4 x i32>, <4 x i32>, ptr)
declare void @g()
define void @f(ptr %p, ptr %o) {
  %r = call {<4 x i32>, <4 x i32>, <4 x i32>, <4 x i32>} @llvm.aarch64.neon.ld4.v4i32.p0(ptr %p)
  call void @g()
  %a = extractvalue {<4 x i32>, <4 x i32>, <4 x i32>, <4 x i32>} %r, 0
  %b = extractvalue {<4 x i32>, <4 x i32>, <4 x i32>, <4 x i32>} %r, 1
  %c = extractvalue {<4 x i32>, <4 x i32>, <4 x i32>, <4 x i32>} %r, 2
  %d = extractvalue {<4 x i32>, <4 x i32>, <4 x i32>, <4 x i32>} %r, 3
  call void @llvm.aarch64.neon.st4.v4i32.p0(<4 x i32> %a, <4 x i32> %b, <4 x i32> %c, <4 x i32> %d, ptr %o)
  ret void
}
"#;

fn count(
    asm: &str,
    mnemonic: &str,
) -> usize {
    asm.lines().filter(|line| line.trim_start().starts_with(mnemonic)).count()
}

/// The `q` registers the accesses of `mnemonic` and its pair form name.
fn quads(
    asm: &str,
    single: &str,
    pair: &str,
) -> usize {
    count(asm, &format!("{single} q")) + 2 * count(asm, &format!("{pair} q"))
}

#[test]
fn a_tuple_live_across_a_call_is_spilled_member_by_member() {
    let asm = driver::assembly(IR, Platform::Linux, true).expect("lowers");
    assert_eq!((count(&asm, "ld4"), count(&asm, "st4")), (1, 1), "{asm}");
    let (stored, loaded) = (quads(&asm, "str", "stp"), quads(&asm, "ldr", "ldp"));
    assert!(stored == 4 && loaded == 4, "the four members go to the frame and back ({stored}, {loaded}): {asm}");
}
