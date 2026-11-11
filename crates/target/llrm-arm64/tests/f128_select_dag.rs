//! `select` of `fp128` on the DAG path alone.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// `F128CSEL` had no instruction, so
/// `tests/check/c/unroll_outer_of_a_loop_is_priced32.c` fell back to the hand
/// selector at O1-Oz. The pin splits the block (`EmitF128CSEL`): `b.cc` to an
/// empty block, a phi in the join.
#[test]
fn an_fp128_select_is_a_branch_around_a_phi_not_a_refusal() {
    let ir = format!(
        "{LAYOUT}define fp128 @f(i32 %a, fp128 %x, fp128 %y) {{\n  %c = icmp slt i32 %a, 0\n  %r = select i1 %c, fp128 %x, fp128 %y\n  %s = fadd fp128 %r, %y\n  ret fp128 %s\n}}\n"
    );
    let asm = driver::assembly(&ir, Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("b.mi") || asm.contains("b.ge") || asm.contains("b.pl") || asm.contains("b.lt"), "{asm}");
    assert!(asm.contains("__addtf3"), "the code after the select stays in the function:\n{asm}");
}

/// A select in a loop splits the loop's block: the loop-carried phis name the
/// join as their predecessor, and a block that fell through to the next one
/// jumps. Under qemu `hi - lo` over 12 values was 160, as gcc.
#[test]
fn two_fp128_selects_in_a_loop_keep_the_loop_carried_values() {
    let ir = format!(
        "{LAYOUT}define fp128 @f(ptr %a, i32 %n) {{\nentry:\n  %f = load fp128, ptr %a\n  br label %loop\nloop:\n  %i = phi i32 [1, %entry], [%i1, %loop]\n  %lo = phi fp128 [%f, %entry], [%lo1, %loop]\n  %hi = phi fp128 [%f, %entry], [%hi1, %loop]\n  %p = getelementptr fp128, ptr %a, i32 %i\n  %v = load fp128, ptr %p\n  %c1 = fcmp olt fp128 %v, %lo\n  %lo1 = select i1 %c1, fp128 %v, fp128 %lo\n  %c2 = fcmp ogt fp128 %v, %hi\n  %hi1 = select i1 %c2, fp128 %v, fp128 %hi\n  %i1 = add i32 %i, 1\n  %d = icmp slt i32 %i1, %n\n  br i1 %d, label %loop, label %exit\nexit:\n  %r = fsub fp128 %hi1, %lo1\n  ret fp128 %r\n}}\n"
    );
    let asm = driver::assembly(&ir, Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("__subtf3"), "{asm}");
}
