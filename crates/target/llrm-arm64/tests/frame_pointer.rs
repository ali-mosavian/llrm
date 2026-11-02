//! The `"frame-pointer"` attribute on arm64 (`AArch64FrameLowering::hasFP`).

use llrm_arm64::driver::{self, IselMode};
use llrm_arm64::machine::Platform;

fn asm(attr: &str) -> String {
    let ir = format!(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i32 @leaf(i32 %a) uwtable {attr} {{\n  %x = alloca [4 x i32], align 16\n  store volatile i32 %a, ptr %x\n\
  %v = load volatile i32, ptr %x\n  ret i32 %v\n}}\n"
    );
    driver::set_isel_mode(IselMode::Dag);
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

/// The attribute was ignored: a leaf with `"frame-pointer"="all"` was left
/// frameless (no `stp x29, x30`, no `add x29, sp`) where the pin makes the
/// frame record (`sub sp, #32; stp x29, x30, [sp, #16]; add x29, sp, #16`);
/// Apple's unwinder needs the chain.
#[test]
fn a_leaf_with_frame_pointer_all_makes_a_frame_record() {
    let asm = asm("\"frame-pointer\"=\"all\"");
    assert!(asm.contains("stp x29, x30, [sp, #16]") && asm.contains("add x29, sp, #16"), "{asm}");
}

/// `non-leaf` keeps the frame only where the function calls.
#[test]
fn a_leaf_with_frame_pointer_non_leaf_stays_frameless() {
    assert!(!asm("\"frame-pointer\"=\"non-leaf\"").contains("x29"));
}
