//! `AArch64MIPeepholeOpt::visitORR`: a zero extend of a 32-bit result is free.

use llrm_arm64::driver::{self, IselMode};
use llrm_arm64::machine::Platform;

fn asm(ir: &str) -> String {
    driver::set_isel_mode(IselMode::Dag);
    driver::assembly(ir, Platform::Linux, true).expect("lowers")
}

const DATALAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// `zext (add i32 x, 7)` was `add w; orr w, wzr, w` (a `mov w, w`) where the
/// pin has the `add w` alone: a 32-bit write clears the upper half already.
#[test]
fn a_zero_extend_of_a_32_bit_add_is_no_instruction() {
    let asm = asm(&format!(
        "{DATALAYOUT}define i64 @f(i32 %x) {{\n  %a = add i32 %x, 7\n  %z = zext i32 %a to i64\n  ret i64 %z\n}}\n"
    ));
    assert!(asm.contains("add w") && !asm.contains("orr w"), "{asm}");
}

/// An argument is not a 32-bit result: the caller may leave the upper half
/// set, so its zero extend stays.
#[test]
fn a_zero_extend_of_an_argument_stays() {
    let asm = asm(&format!("{DATALAYOUT}define i64 @f(i32 %x) {{\n  %z = zext i32 %x to i64\n  ret i64 %z\n}}\n"));
    assert!(asm.contains("orr w") || asm.contains("mov w"), "{asm}");
}
