//! CodeGenPrepare promotes `sext (add (load i8))` towards the load when the target says so
//! (`EnableExtLdPromotion`, true on AArch64 at the pin): `ldrsb` + `add`. Without it the load
//! zero-extends and the sign extension is a separate `sbfm`: `ldrb`, `add`, `sbfm`.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const BODY: &str = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i32 @f(ptr %p) {
  %l = load i8, ptr %p
  %a = add nsw i8 %l, 1
  %e = sext i8 %a to i32
  ret i32 %e
}
define i64 @g(ptr %p) {
  %l = load i16, ptr %p
  %a = sub nsw i16 %l, 3
  %e = sext i16 %a to i64
  ret i64 %e
}
"#;

#[test]
fn a_sign_extended_narrow_sum_of_a_load_is_made_from_a_sign_extending_load() {
    let asm = driver::assembly(BODY, Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("ldrsb") && asm.contains("ldrsh"), "{asm}");
    assert!(!asm.contains("sbfm"), "{asm}");
}
