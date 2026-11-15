//! The add or subtract of two widened vectors is one `saddl`, `uaddl`,
//! `ssubl` or `usubl`. `add (sext a), (sext b)` stopped at "machine opcode
//! SADDLv4i16_v4i32 has no arm64 instruction yet" once instcombine folded the
//! zero start of a widened reduction (tile -O2).

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn assembly(
    ext: &str,
    op: &str,
) -> String {
    let ir = format!(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f(ptr %p, ptr %q, ptr %o) {{
  %a = load <4 x i16>, ptr %p, align 8
  %b = load <4 x i16>, ptr %q, align 8
  %x = {ext} <4 x i16> %a to <4 x i32>
  %y = {ext} <4 x i16> %b to <4 x i32>
  %r = {op} <4 x i32> %x, %y
  store <4 x i32> %r, ptr %o, align 16
  ret void
}}
"#
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

#[test]
fn a_sum_of_two_widened_vectors_is_one_long_add() {
    assert!(assembly("sext", "add").contains("saddl"));
    assert!(assembly("zext", "add").contains("uaddl"));
}

#[test]
fn a_difference_of_two_widened_vectors_is_one_long_subtract() {
    assert!(assembly("sext", "sub").contains("ssubl"));
    assert!(assembly("zext", "sub").contains("usubl"));
}
