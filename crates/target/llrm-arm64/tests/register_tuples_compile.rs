//! A register file with tuple registers (`v0_v1`, ...) is still a file the
//! lane tables of `llrm-codegen` can be built for: it made every arm64
//! compile panic ("289 lanes of a register file are more than a mask holds",
//! 175 of 215 identity files) once tuples were listed as roots.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

#[test]
fn a_function_compiles_with_tuple_registers_in_the_file() {
    let ir = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i32 @f(i32 %a, i32 %b) {
  %s = add i32 %a, %b
  ret i32 %s
}
"#;
    let asm = driver::assembly(ir, Platform::Linux, true).expect("lowers");
    assert!(asm.contains("add"), "{asm}");
}
