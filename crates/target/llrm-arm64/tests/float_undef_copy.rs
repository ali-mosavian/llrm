//! A float phi whose incoming value is undefined, on the DAG path alone.

use llrm_arm64::driver::{self, IselMode};
use llrm_arm64::machine::Platform;

/// The copy of an undefined float into a register was refused ("a float copy of
/// a constant"), so every function with one fell back to the hand selector:
/// `float_in_place_update` (`oldrho`) at -O1 and -Os.
#[test]
fn a_float_phi_with_an_undefined_incoming_value_is_selected_by_the_dag() {
    let ir = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define double @f(ptr %a, i64 %n) {\n\
entry:\n  br label %loop\n\
loop:\n  %i = phi i64 [ 0, %entry ], [ %i1, %loop ]\n  %s = phi double [ undef, %entry ], [ %s1, %loop ]\n\
  %p = getelementptr double, ptr %a, i64 %i\n  %v = load double, ptr %p\n  %s1 = fadd double %s, %v\n\
  %i1 = add i64 %i, 1\n  %c = icmp eq i64 %i1, %n\n  br i1 %c, label %done, label %loop\n\
done:\n  ret double %s1\n}\n";
    driver::set_isel_mode(IselMode::Dag);
    let asm = driver::assembly(ir, Platform::Linux, true).expect("the DAG path lowers an undefined float copy");
    assert!(asm.contains("fmov"), "{asm}");
}
