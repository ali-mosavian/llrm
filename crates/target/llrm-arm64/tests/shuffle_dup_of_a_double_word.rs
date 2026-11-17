//! A splat of one lane of a 64-bit vector is `dup` of the lane.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// A `<4 x i32>` widened to `<8 x i32>` with an undefined upper half was a
/// store of an interleave of factor four (`st4` of `<2 x i32>` members); a
/// member that splats lane 1 of a `<2 x i32>` was refused by the legalizer
/// ("custom lowering of VectorShuffle"). The pin widens the 64-bit source to
/// 128 bits and takes `DUPLANE32` (`constructDup`).
#[test]
fn a_widening_shuffle_with_an_undefined_upper_half_lowers() {
    let asm = driver::assembly(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define void @f(ptr %p, ptr %e) {
  %hi = load <4 x i32>, ptr %p
  %wide = shufflevector <4 x i32> %hi, <4 x i32> poison, <8 x i32> <i32 0, i32 1, i32 2, i32 3, i32 poison, i32 poison, i32 poison, i32 poison>
  store <8 x i32> %wide, ptr %e, align 16
  ret void
}
"#,
        Platform::Linux,
        Level::O2,
    )
    .expect("lowers");
    assert!(asm.contains("st"), "{asm}");
}
