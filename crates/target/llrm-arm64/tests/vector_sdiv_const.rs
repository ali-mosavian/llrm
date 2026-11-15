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

#[test]
fn a_v4i32_signed_quotient_by_a_power_of_two_is_cmlt_usra_sshr() {
    let asm = assembly("4", "%r = sdiv <4 x i32> %x, <i32 8, i32 8, i32 8, i32 8>");
    for wanted in ["cmlt", "usra", "sshr"] {
        assert!(asm.contains(wanted), "{wanted}: {asm}");
    }
    assert!(!asm.contains("smull"), "{asm}");
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

/// The count-down sum of `(3 * n) % 11` into an `i64` (`recsum`, 14,007
/// instructions at -O2 against clang's 7,780) stayed scalar: at VF 2, the
/// widest type's, the `<2 x i32>` remainder had no price, so "not beneficial".
#[test]
fn a_sum_of_signed_remainders_into_a_wider_accumulator_is_vectorized() {
    let ir = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i64 @f(i32 %n) {
entry:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %exit, label %loop
loop:
  %acc = phi i64 [ 0, %entry ], [ %acc1, %loop ]
  %i = phi i32 [ %n, %entry ], [ %i1, %loop ]
  %m = mul nsw i32 %i, 3
  %r = srem i32 %m, 11
  %w = sext i32 %r to i64
  %acc1 = add i64 %acc, %w
  %i1 = add i32 %i, -1
  %c = icmp eq i32 %i1, 0
  br i1 %c, label %exit, label %loop
exit:
  %res = phi i64 [ 0, %entry ], [ %acc1, %loop ]
  ret i64 %res
}
"#;
    let asm = driver::assembly(ir, Platform::Linux, true).expect("lowers");
    assert!(asm.contains("smull"), "{asm}");
    assert!(asm.contains(".4s") || asm.contains(".2s"), "{asm}");
}
