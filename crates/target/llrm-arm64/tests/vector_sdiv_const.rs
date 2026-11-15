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

fn loop_assembly(divisor: i32) -> String {
    let ir = format!(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f(ptr noalias %p, ptr noalias %q, i64 %n) {{
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %i1, %loop ]
  %a = getelementptr i32, ptr %p, i64 %i
  %x = load i32, ptr %a, align 4
  %d = sdiv i32 %x, {divisor}
  %b = getelementptr i32, ptr %q, i64 %i
  store i32 %d, ptr %b, align 4
  %i1 = add i64 %i, 1
  %c = icmp eq i64 %i1, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}}
"#
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

/// A loop of `sdiv i32 %x, 11` stayed scalar (`smull` of one lane per
/// iteration): the vectorizer widened only unsigned divides.
#[test]
fn a_loop_of_signed_divides_by_a_constant_is_vectorized() {
    let asm = loop_assembly(11);
    assert!(asm.contains("smull2") && asm.contains("uzp2"), "{asm}");
}

/// A negative divisor goes the same way (the pin does not price it apart).
#[test]
fn a_loop_of_signed_divides_by_a_negative_constant_is_vectorized() {
    let asm = loop_assembly(-7);
    assert!(asm.contains("smull2"), "{asm}");
}
