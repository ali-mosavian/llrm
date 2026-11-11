//! `AArch64CondBrTuning`: a `cbz`/`cbnz` on the result of an add, subtract or
//! and made in the same block is a flag-setting instruction and a `b.cond`.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn assembly(ir: &str) -> String {
    driver::assembly(ir, Platform::Linux, true).expect("lowers")
}

const DATALAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// `sub w0, w0, #1; cbnz w0` stayed a `cbnz` where the pin makes
/// `subs w0, w0, #1; b.ne`: the flag-setting subtract leaves the value in the
/// register and the branch off the compare-and-branch unit.
#[test]
fn a_compare_and_branch_on_a_difference_is_the_flag_setting_subtract() {
    let ir = format!(
        "{DATALAYOUT}declare void @g(i32)\ndeclare void @h(i32)\ndefine void @f(i32 %n) {{\n  %a = add nsw i32 %n, -1\n  \
         %c = icmp eq i32 %a, 0\n  br i1 %c, label %x, label %y\nx:\n  call void @g(i32 %a)\n  ret void\ny:\n  call void @h(i32 %a)\n  ret void\n}}\n"
    );
    let asm = assembly(&ir);
    assert!(asm.contains("subs"), "{asm}");
    assert!(asm.contains("b.ne") || asm.contains("b.eq"), "{asm}");
    assert!(!asm.contains("cbz") && !asm.contains("cbnz"), "{asm}");
}

/// A loop counter's `sub` is read by the next trip's phi as well as the
/// branch: the pin runs the pass in SSA, where that `sub` is its register's
/// one definition. After phi elimination the counter has two definitions and
/// the loop kept `sub; cbnz`.
#[test]
fn a_loop_counter_decrement_sets_the_flags_the_latch_reads() {
    let ir = format!(
        "{DATALAYOUT}declare void @g()\ndefine void @f(i32 %n) {{\nentry:\n  br label %l\nl:\n  %i = phi i32 [ %n, %entry ], [ %d, %l ]\n  \
         call void @g()\n  %d = sub i32 %i, 1\n  %c = icmp eq i32 %d, 0\n  br i1 %c, label %x, label %l\nx:\n  ret void\n}}\n"
    );
    let asm = assembly(&ir);
    assert!(asm.contains("subs"), "{asm}");
    assert!(!asm.contains("cbz") && !asm.contains("cbnz"), "{asm}");
}

/// `sdiv; mul; subs; b.ne` where the pin keeps `sdiv; msub; cbnz` (recgcd -O2
/// +8 B): the pass ran before `mul; sub` became an `msub`, which has no
/// flag-setting form; the pin runs it after MachineCombiner.
#[test]
fn a_remainder_tested_against_zero_stays_a_msub() {
    let ir = format!(
        "{DATALAYOUT}declare void @g()\ndefine void @f(i32 %a, i32 %b) {{\n  %r = srem i32 %a, %b\n  %c = icmp eq i32 %r, 0\n  \
         br i1 %c, label %x, label %y\nx:\n  ret void\ny:\n  call void @g()\n  ret void\n}}\n"
    );
    let asm = assembly(&ir);
    assert!(asm.contains("msub"), "{asm}");
    assert!(!asm.contains("subs"), "{asm}");
}
