//! How a float constant is made when optimizing for size.

use llrm_arm64::driver::{self, IselMode};
use llrm_arm64::machine::Platform;

fn asm(attrs: &str) -> String {
    let ir = format!(
        "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define float @f(float %x) {attrs} {{\n  %r = fmul float %x, 0x3FB99999A0000000\n  ret float %r\n}}\n"
    );
    driver::set_isel_mode(IselMode::Dag);
    driver::assembly(&ir, Platform::Darwin, true).expect("lowers")
}

/// DEVIATE FP1: the pin sends a float needing two `mov`s to the constant pool
/// at `-Os`; here it stays `mov; movk; fmov` (12 B, no data) because the pool's
/// 8 data bytes made `floats` +20 B and `recfloat` +8 B.
#[test]
fn a_float_needing_two_moves_is_built_at_size_too() {
    for attrs in ["", "optsize"] {
        let asm = asm(attrs);
        assert!(asm.contains("movk") && !asm.contains("lCPI"), "{attrs}: {asm}");
    }
}

/// A double that is exact as a float (and needs three `mov`s) was pooled as a float, so the legalizer put it in the pool as a
/// float and read it with an extending load, which AArch64 expands: the
/// build failed (`CannotSelect("Load")`, recfloat and floats at `-Os`). The
/// pin shrinks only for a target whose extending load is legal.
#[test]
fn a_double_exact_as_a_float_is_a_double_in_the_pool() {
    let ir = "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define double @f(double %x) optsize {\n  %r = fmul double %x, 0x40934A4580000000\n  ret double %r\n}\n";
    driver::set_isel_mode(IselMode::Dag);
    let asm = driver::assembly(ir, Platform::Darwin, true).expect("lowers");
    assert!(asm.contains("ldr d") && !asm.contains("fcvt"), "{asm}");
}
