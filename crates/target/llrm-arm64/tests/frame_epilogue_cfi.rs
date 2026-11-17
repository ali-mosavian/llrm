//! The epilogue's `.cfi_restore`s are stated once, before the return, in the
//! pin's callee-saved order (`AArch64EpilogueEmitter::finalizeEpilogue`).
//! Only an asynchronous table (`uwtable`) has them.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const SOURCE: &str = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare i32 @g(i32)
define i32 @f(i32 %a) uwtable "frame-pointer"="non-leaf" {
  %x = call i32 @g(i32 %a)
  %y = call i32 @g(i32 %x)
  %z = call i32 @g(i32 %y)
  %s = add i32 %x, %y
  %t = add i32 %s, %z
  ret i32 %t
}
"#;

/// fib/queens/recgcd (16 of 77 FDEs, bench -O2): the restores followed each
/// `ldp` (`restore x19, x20` after the first, `x30, x29` after the pop), where
/// the pin puts all of them after the last, behind `def_cfa_offset 0`: fib's
/// FDE was 0x34 bytes against the pin's 0x28.
#[test]
fn the_restores_follow_the_last_load_in_the_pins_order() {
    let asm = driver::assembly(SOURCE, Platform::Linux, Level::O2).expect("lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let last_load = lines.iter().rposition(|line| line.starts_with("ldp")).expect("a load");
    let after: Vec<&str> = lines[last_load + 1..].iter().copied().filter(|line| line.starts_with(".cfi")).collect();
    assert_eq!(
        after,
        [".cfi_def_cfa_offset 0", ".cfi_restore w19", ".cfi_restore w20", ".cfi_restore w30", ".cfi_restore w29"],
        "{asm}"
    );
    let restores = lines.iter().filter(|line| line.starts_with(".cfi_restore")).count();
    assert_eq!(restores, 4, "{asm}");
}
