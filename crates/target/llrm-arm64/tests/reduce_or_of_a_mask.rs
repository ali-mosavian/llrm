//! `reduce.or` of a compare of eight halfwords: the any-of of an early-exit loop.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// The mask of a `<8 x i16>` compare is a `v8i8` after type legalization, and
/// its reduction had no lowering (the 64-bit types were `Expand`): the vector
/// went through a stack slot byte by byte, and the loop that used it failed
/// to select ("CannotSelect Target(803)", `tbnz`). The pin's reductions of
/// the 64-bit types are `LowerVECREDUCE` too: the vector is one scalar.
#[test]
fn an_any_of_a_halfword_compare_is_a_scalar_test_of_the_mask() {
    let asm = driver::assembly(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define i1 @f(ptr %p, ptr %q) {
  %x = load <8 x i16>, ptr %p, align 16
  %y = load <8 x i16>, ptr %q, align 16
  %c = icmp ult <8 x i16> %x, %y
  %r = call i1 @llvm.vector.reduce.or.v8i1(<8 x i1> %c)
  ret i1 %r
}
declare i1 @llvm.vector.reduce.or.v8i1(<8 x i1>)
"#,
        Platform::Linux,
        Level::O2,
    )
    .expect("lowers");
    // `getVectorBitwiseReduce`'s boolean case: the mask sign-extended to
    // halfwords is `umaxv` (pin llc: `cmhi`, `umaxv h`, `fmov`).
    assert!(asm.contains("cmhi") && asm.contains("umaxv"), "{asm}");
    assert!(!asm.contains("[sp"), "{asm}");
}
