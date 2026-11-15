//! A signed divide of a vector by a constant is the multiply-high sequence
//! (`BuildSDIV`). Each case first failed: a vector `sdiv`/`srem` was
//! `CannotSelect`, LICM panicked on it, and `shrn` had no encoding.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn assembly(
    lanes: &str,
    body: &str,
) -> String {
    let (n, bytes) = if lanes == "4" { (4, 16) } else { (2, 8) };
    let ir = format!(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f(ptr %p, ptr %q) {{
  %x = load <{n} x i32>, ptr %p, align {bytes}
  {body}
  store <{n} x i32> %r, ptr %q, align {bytes}
  ret void
}}
"#
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

#[test]
fn a_v4i32_signed_remainder_by_a_constant_is_a_multiply_high() {
    let asm = assembly("4", "%r = srem <4 x i32> %x, <i32 11, i32 11, i32 11, i32 11>");
    for wanted in ["smull2", "smull ", "uzp2", "sshr", "usra"] {
        assert!(asm.contains(wanted), "{wanted}: {asm}");
    }
    assert!(!asm.contains("sdiv"), "{asm}");
}

/// `srem <2 x i32>` stopped at "machine opcode SHRNv2i32_shift has no arm64
/// instruction yet": the 64-bit multiply-high narrows with `shrn`.
#[test]
fn a_v2i32_signed_remainder_by_a_constant_narrows_with_shrn() {
    let asm = assembly("2", "%r = srem <2 x i32> %x, <i32 11, i32 11>");
    for wanted in ["smull", "shrn", "sshr", "usra"] {
        assert!(asm.contains(wanted), "{wanted}: {asm}");
    }
}
