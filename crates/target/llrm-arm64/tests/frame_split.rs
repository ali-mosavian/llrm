//! A frame of 512 bytes or more pushes the saved registers first and takes the
//! locals after (`shouldCombineCSRLocalStackBump` is off): a 4 KB buffer was
//! one `sub sp` of 4128 bytes made through `x16`, and the saves were stored
//! through `x16`/`x17` addresses (quicksort -Os: 8 instructions more than clang).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn asm() -> String {
    driver::assembly(include_str!("data/frame_split.ll"), Platform::Linux, Level::O2).expect("lowers")
}

#[test]
fn a_large_frame_pushes_the_saved_registers_before_the_locals() {
    let asm = asm();
    let lines: Vec<&str> = asm.lines().map(str::trim).filter(|line| !line.starts_with('.')).collect();
    let push = lines.iter().position(|line| line.starts_with("stp x29, x30, [sp, #-")).expect("a push");
    let locals = lines.iter().position(|line| line.starts_with("sub sp, sp, #1, lsl #12")).expect("the locals");
    assert!(push < locals, "{asm}");
}

#[test]
fn a_large_frame_needs_no_scratch_register() {
    let asm = asm();
    assert!(!asm.contains("x16") && !asm.contains("x17"), "{asm}");
}

#[test]
fn a_large_frame_gives_the_locals_back_before_the_registers() {
    let asm = asm();
    let lines: Vec<&str> = asm.lines().map(str::trim).filter(|line| !line.starts_with('.')).collect();
    let give = lines.iter().position(|line| line.starts_with("add sp, sp, #1, lsl #12")).expect("the locals back");
    let pop = lines.iter().position(|line| line.starts_with("ldp x29, x30, [sp], #")).expect("a pop");
    assert!(give < pop, "{asm}");
}
