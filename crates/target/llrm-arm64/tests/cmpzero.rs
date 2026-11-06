//! `AArch64InstrInfo::substituteCmpToZero`: a compare with zero of a value made
//! in the same block goes into the instruction that made it.

use llrm_arm64::driver::{self, IselMode};
use llrm_arm64::machine::Platform;

fn assembly(ir: &str) -> String {
    driver::set_isel_mode(IselMode::Dag);
    driver::assembly(ir, Platform::Linux, true).expect("lowers")
}

const DATALAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// `add; cmp #0; csel mi` stayed three instructions where the pin makes
/// `adds; csel mi`.
#[test]
fn a_compare_with_zero_of_a_sum_is_the_flag_setting_add() {
    let ir = format!(
        "{DATALAYOUT}define i32 @f(i32 %a, i32 %b, i32 %x, i32 %y) {{\n  %s = add i32 %a, %b\n  %c = icmp slt i32 %s, 0\n  \
         %r = select i1 %c, i32 %x, i32 %y\n  %t = add i32 %r, %s\n  ret i32 %t\n}}\n"
    );
    let asm = assembly(&ir);
    assert!(asm.contains("adds"), "{asm}");
    assert!(!asm.contains("cmp"), "{asm}");
}

/// `gt` reads V, which the compare of zero clears and `adds` may set: the
/// compare stays.
#[test]
fn a_reader_of_the_overflow_flag_keeps_the_compare() {
    let ir = format!(
        "{DATALAYOUT}define i32 @f(i32 %a, i32 %b, i32 %x, i32 %y) {{\n  %s = add i32 %a, %b\n  %c = icmp sgt i32 %s, 0\n  \
         %r = select i1 %c, i32 %x, i32 %y\n  %t = add i32 %r, %s\n  ret i32 %t\n}}\n"
    );
    let asm = assembly(&ir);
    assert!(!asm.contains("adds"), "{asm}");
}

/// `switch i8 (trunc nuw i32 %x)` compared the byte after an `and #0xff`; the
/// pin's `SimplifySetCC` compares `%x` itself (the switch of `jpeg_fixed` -Os,
/// +4 B once `visitSwitchInst` narrowed its condition).
#[test]
fn a_switch_on_a_trunc_nuw_compares_the_wide_value() {
    let ir = format!(
        "{DATALAYOUT}declare void @a()\ndeclare void @b()\ndeclare void @c()\ndeclare void @d()\n\
         define void @f(i32 %x) {{\n  %t = trunc nuw i32 %x to i8\n  switch i8 %t, label %o [ i8 -37, label %l1 i8 -60, label %l2 \
         i8 -64, label %l3 ]\nl1:\n  call void @a()\n  ret void\nl2:\n  call void @b()\n  ret void\nl3:\n  call void @c()\n  ret void\n\
         o:\n  call void @d()\n  ret void\n}}\n"
    );
    let asm = assembly(&ir);
    assert!(!asm.contains("and "), "{asm}");
}
