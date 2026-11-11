//! `((x ^ y) & m) ^ y` is `bic` + `and` + `orr` (pin `hasAndNot`); with the
//! hook off it stayed `eor`, `and`, `eor` and no `bic` was made.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const IR: &str = r#"target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i32 @merge(i32 %x, i32 %y, i32 %m) {
  %d = xor i32 %x, %y
  %a = and i32 %d, %m
  %r = xor i32 %a, %y
  ret i32 %r
}
"#;

#[test]
fn a_masked_merge_is_made_of_bic_and_orr() {
    let asm = driver::assembly(IR, Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("bic"), "{asm}");
    assert!(!asm.contains("eor"), "{asm}");
}
