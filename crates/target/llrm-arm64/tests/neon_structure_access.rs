//! `llvm.aarch64.neon.ld2/ld3/ld4/st2/st3/st4` are `ldN`/`stN`
//! (`AArch64DAGToDAGISel::SelectLoad`, `SelectStore`): the interleaved
//! accesses of `InterleavedAccess` need them, and without them `textfill -O2`
//! runs 32009 instructions to the pin's 4120.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn assembly(body: &str) -> String {
    let ir = format!(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare {{<4 x i32>, <4 x i32>}} @llvm.aarch64.neon.ld2.v4i32.p0(ptr)
declare void @llvm.aarch64.neon.st2.v4i32.p0(<4 x i32>, <4 x i32>, ptr)
{body}
"#
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

/// A `ld2` read and a `st2` of its members: an `ld2` and an `st2`, the pair in
/// consecutive registers.
#[test]
fn ld2_then_st2_of_its_members_is_ld2_and_st2() {
    let asm = assembly(
        r#"define void @f(ptr %p, ptr %o) {
  %r = call {<4 x i32>, <4 x i32>} @llvm.aarch64.neon.ld2.v4i32.p0(ptr %p)
  %a = extractvalue {<4 x i32>, <4 x i32>} %r, 0
  %b = extractvalue {<4 x i32>, <4 x i32>} %r, 1
  %s = add <4 x i32> %a, %b
  call void @llvm.aarch64.neon.st2.v4i32.p0(<4 x i32> %s, <4 x i32> %b, ptr %o)
  ret void
}"#,
    );
    println!("{asm}");
    assert!(asm.contains("ld2"), "{asm}");
    assert!(asm.contains("st2"), "{asm}");
}
