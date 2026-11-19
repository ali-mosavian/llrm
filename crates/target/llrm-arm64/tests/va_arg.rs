//! Apple's `va_arg` was refused ("`va_arg` is not an instruction of MIR's
//! subset") until the driver wrote it out in the text. The selector now takes
//! it: the list is read, the value loaded through it, the pointer stored a
//! slot on.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const READ: &str = "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i32 @f(ptr %ap) {\n  %v = va_arg ptr %ap, i32\n  ret i32 %v\n}\n";

#[test]
fn test_va_arg_loads_through_the_list_and_steps_one_slot() {
    let asm = driver::assembly(READ, Platform::Darwin, Level::O2).unwrap_or_else(|why| panic!("{why}"));
    let body: Vec<&str> = asm.lines().map(str::trim).collect();
    let at = |prefix: &str, suffix: &str| body.iter().position(|l| l.starts_with(prefix) && l.ends_with(suffix));
    let (load, step, store, value) =
        (at("ldr x", ", [x0]"), at("add x", ", #8"), at("str x", ", [x0]"), at("ldr w0, [x", "]"));
    assert!(load < step && step < store && value.is_some(), "{asm}");
    assert!(load.is_some() && step.is_some() && store.is_some(), "{asm}");
}
