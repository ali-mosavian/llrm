//! `ShrinkDemandedOp` (TargetLowering.cpp L641): an operation whose result is
//! only read in its low half is made at the narrower type.

use llrm_arm64::driver::{self, IselMode};
use llrm_arm64::machine::Platform;

/// `and (add nuw i, 7), 0xffffffff` into a `mul` stayed `add x; and
/// #0xffffffff; mul` where the pin makes `add w` (the write clears the upper
/// half): two `add w`, no `and`.
#[test]
fn a_masked_add_is_made_at_the_narrow_width() {
    let ir = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i64 @f(i64 %i, i64 %b, i64 %d, i64 %acc) {\n\
  %a1 = add nuw i64 %i, 1\n  %a2 = and i64 %a1, 4294967295\n  %m1 = mul nsw i64 %a2, %b\n\
  %c7 = add nuw i64 %i, 7\n  %c8 = and i64 %c7, 4294967295\n  %m2 = mul nsw i64 %c8, %d\n\
  %s1 = add i64 %m1, %acc\n  %s2 = add i64 %s1, %m2\n  ret i64 %s2\n}\n";
    driver::set_isel_mode(IselMode::Dag);
    let asm = driver::assembly(ir, Platform::Linux, true).expect("lowers");
    assert_eq!(asm.matches("add w").count(), 2, "{asm}");
    assert!(!asm.contains("and x"), "{asm}");
}
