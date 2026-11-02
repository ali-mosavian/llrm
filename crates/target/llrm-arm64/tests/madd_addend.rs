//! `MachineCombiner` madd patterns on arm64
//! (`AArch64InstrInfo::getMaddPatterns`).

use llrm_arm64::driver::{self, IselMode, Level};
use llrm_arm64::machine::Platform;

/// The second product of `acc + a*b + c*d` in a loop stayed `mul` + `add` when
/// the `add` of the first sum sat between the second `mul` and the final
/// `add`: the addend was judged "redefined" though the final `add` reads it
/// where it stands. The pin fuses both. Missed fusion only; the code was
/// right (`fpbench` -Os, one extra `add` per trip).
#[test]
fn both_products_of_a_sum_become_madd_whatever_the_order() {
    let ir = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i64 @f(ptr %p, ptr %q) optsize {\n\
entry:\n  br label %l\n\
l:\n  %i = phi i64 [ %n, %l ], [ 0, %entry ]\n  %acc = phi i64 [ %s2, %l ], [ 0, %entry ]\n\
  %pa = getelementptr inbounds nuw [4 x i8], ptr %p, i64 %i\n  %va = load float, ptr %pa\n\
  %fa = fmul float %va, 6.400000e+01\n  %ia = fptosi float %fa to i64\n\
  %a1 = add nuw nsw i64 %i, 1\n  %m1 = mul nsw i64 %a1, %ia\n\
  %pb = getelementptr inbounds nuw [4 x i8], ptr %q, i64 %i\n  %vb = load float, ptr %pb\n\
  %fb = fmul float %vb, 6.400000e+01\n  %ib = fptosi float %fb to i64\n\
  %c7 = add nuw nsw i64 %i, 7\n  %m2 = mul nsw i64 %c7, %ib\n\
  %s1 = add i64 %m1, %acc\n  %s2 = add i64 %s1, %m2\n  %n = add nuw nsw i64 %i, 1\n\
  %c = icmp eq i64 %n, 6\n  br i1 %c, label %x, label %l\n\
x:\n  ret i64 %s2\n}\n";
    driver::set_isel_mode(IselMode::Dag);
    let asm = driver::assembly(ir, Platform::Linux, Level::Os).expect("lowers");
    assert_eq!(asm.matches("madd").count(), 2, "{asm}");
    // A product on its own is a `madd` with the zero register as the addend.
    assert!(!asm.contains(", xzr\n"), "{asm}");
}
