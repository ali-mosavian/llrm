//! Where the copies that carry phi values go.

use llrm_arm64::driver::{self, IselMode};
use llrm_arm64::machine::Platform;

/// A loop carrying a double ran `fmov d3, d4; b loop` in an edge block of its
/// own on every trip (6 instructions a trip, the pin 5): the pin copies before
/// the latch's branch when the value is dead on the other path
/// (`PHIElimination::SplitPHIEdges`) and never splits a back edge.
#[test]
fn a_loop_carried_copy_is_in_the_latch_not_in_an_edge_block() {
    let ir = r#"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
define double @f(i32 %n) {
entry:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %exit, label %loop
loop:
  %x = phi double [ 100.0, %entry ], [ %x2, %loop ]
  %acc = phi double [ 0.0, %entry ], [ %acc2, %loop ]
  %i = phi i32 [ %n, %entry ], [ %i2, %loop ]
  %m = fmul double %x, 5.000000e-01
  %x2 = fadd double %m, 1.000000e+00
  %acc2 = fadd double %acc, %x
  %i2 = sub i32 %i, 1
  %c = icmp eq i32 %i2, 0
  br i1 %c, label %exit, label %loop
exit:
  %r = phi double [ 0.0, %entry ], [ %acc2, %loop ]
  ret double %r
}
"#;
    driver::set_isel_mode(IselMode::Dag);
    let asm = driver::assembly(ir, Platform::Darwin, true).expect("lowers");
    // Premise: the loop carries a value in a copy.
    assert!(asm.contains("fmov d"), "{asm}");
    // The back edge is the latch's own conditional branch.
    let jumps = asm.lines().filter(|l| l.trim_start().starts_with("b ")).count();
    assert_eq!(jumps, 0, "{asm}");
}

/// The exit reads the loop-carried value as it was on entry to the last trip,
/// so the latch's copy into that register would overwrite it (the lost copy):
/// the edge copies into a temporary per phi, as the pin does, and the loop's
/// back edge stays the latch's own branch.
#[test]
fn a_loop_carried_value_the_exit_reads_needs_no_edge_block() {
    let ir = r#"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
define double @f(i32 %n) {
entry:
  br label %loop
loop:
  %x = phi double [ 100.0, %entry ], [ %x2, %loop ]
  %i = phi i32 [ %n, %entry ], [ %i2, %loop ]
  %m = fmul double %x, 5.000000e-01
  %x2 = fadd double %m, 1.000000e+00
  %i2 = sub i32 %i, 1
  %c = icmp eq i32 %i2, 0
  br i1 %c, label %exit, label %loop
exit:
  %r = fadd double %x, %x2
  ret double %r
}
"#;
    driver::set_isel_mode(IselMode::Dag);
    let asm = driver::assembly(ir, Platform::Darwin, true).expect("lowers");
    assert!(asm.contains("fmov d"), "{asm}");
    let jumps = asm.lines().filter(|l| l.trim_start().starts_with("b ")).count();
    assert_eq!(jumps, 0, "{asm}");
}
