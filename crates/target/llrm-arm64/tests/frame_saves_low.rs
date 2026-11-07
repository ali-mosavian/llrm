//! DEVIATE ARM64-FRAME-LOW: a small frame with locals pushes its frame record
//! with the stack pointer (`stp [sp, #-N]!`) and pops it the same way.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const SOURCE: &str = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare void @use(ptr)
define void @f() {
  %a = alloca [4 x i64], align 8
  call void @use(ptr %a)
  ret void
}
"#;

fn instructions(asm: &str) -> Vec<&str> {
    asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).map(str::trim).collect()
}

/// queens -Os: `sub sp, sp, #64; stp x29, x30, [sp, #48]; add x29, sp, #48` and
/// `ldp; add sp` against gcc's `stp [sp, #-64]!; mov x29, sp` and `ldp [sp], #64`:
/// two instructions more than gcc in a small frame with a local.
#[test]
fn a_small_frame_with_a_local_pushes_the_record_with_the_stack_pointer() {
    for platform in [Platform::Linux, Platform::Darwin] {
        let asm = driver::assembly(SOURCE, platform, Level::O2).expect("lowers");
        let insts = instructions(&asm);
        assert!(insts.iter().any(|i| i.starts_with("stp x29, x30, [sp, #-48]!")), "{platform:?}:\n{asm}");
        assert!(insts.iter().any(|i| i.starts_with("ldp x29, x30, [sp], #48")), "{platform:?}:\n{asm}");
        assert!(!insts.iter().any(|i| i.starts_with("sub sp") || i.starts_with("add sp")), "{platform:?}:\n{asm}");
    }
}
