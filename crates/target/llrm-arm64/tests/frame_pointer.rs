//! The `"frame-pointer"` attribute on arm64 (`AArch64FrameLowering::hasFP`).

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn asm(attr: &str) -> String {
    on(Platform::Linux, attr)
}

fn on(platform: Platform, attr: &str) -> String {
    let ir = format!(
        "target datalayout = \"{}\"\n\
define i32 @leaf(i32 %a) uwtable {attr} {{\n  %x = alloca [4 x i32], align 16\n  store volatile i32 %a, ptr %x\n\
  %v = load volatile i32, ptr %x\n  ret i32 %v\n}}\n",
        platform.datalayout()
    );
    driver::assembly(&ir, platform, true).expect("lowers")
}

/// The attribute was ignored: a leaf with `"frame-pointer"="all"` was left
/// frameless (no `stp x29, x30`, no `add x29, sp`). On Mach-O the frame is the
/// pin's, which Apple's unwinder reads (`sub sp, #32; stp x29, x30, [sp, #16];
/// add x29, sp, #16`).
#[test]
fn a_leaf_with_frame_pointer_all_makes_a_frame_record() {
    let asm = on(Platform::Darwin, "\"frame-pointer\"=\"all\"");
    assert!(asm.contains("stp x29, x30, [sp, #16]") && asm.contains("add x29, sp, #16"), "{asm}");
}

/// ELF, DEVIATE FR-LOW: the record is lowest and the stack pointer moves in
/// the one `stp` and `ldp` (before: `sub sp, #32` and `add sp, #32` besides).
#[test]
fn on_elf_the_frame_record_is_lowest_and_the_stp_moves_the_stack_pointer() {
    let asm = asm("\"frame-pointer\"=\"all\"");
    assert!(asm.contains("stp x29, x30, [sp, #-32]!") && asm.contains("ldp x29, x30, [sp], #32"), "{asm}");
    assert!(!asm.contains("sub sp"), "{asm}");
}

/// `non-leaf` keeps the frame only where the function calls.
#[test]
fn a_leaf_with_frame_pointer_non_leaf_stays_frameless() {
    assert!(!asm("\"frame-pointer\"=\"non-leaf\"").contains("x29"));
}
