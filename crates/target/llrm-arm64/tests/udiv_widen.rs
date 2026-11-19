//! An `i32` unsigned division by a constant whose magic needs the add fixup is
//! one 64-bit high multiply.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

/// `x % 97` was `umull; lsr; sub; add; lsr; msub` (6 instructions) where the
/// pin widens to `i64` and runs `umulh; msub` (QB `recpow`: 81658 instructions,
/// the pin's loop 3 shorter a trip).
#[test]
fn an_i32_unsigned_remainder_by_97_is_a_wide_umulh() {
    let ir = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i32 @f(i32 %x) {
  %r = urem i32 %x, 97
  ret i32 %r
}
"#;
    let asm = driver::assembly(ir, Platform::Linux, true).expect("lowers");
    assert!(asm.contains("umulh"), "{asm}");
    assert!(!asm.contains("umull"), "{asm}");
}
