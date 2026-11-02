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

/// `0.1f` took `mov; movk; fmov` at `-Os` where the pin loads it from the
/// constant pool (`isFPImmLegal` allows one `mov` when optimizing for size,
/// two otherwise): 12 bytes for the constant against 8 + 4 shared pool bytes
/// (QB float kernels, `.text` +20 B on x_nbody_f).
#[test]
fn a_float_needing_two_moves_comes_from_the_pool_at_size_and_is_built_otherwise() {
    let plain = asm("");
    assert!(plain.contains("movk") && !plain.contains("lCPI"), "{plain}");
    let size = asm("optsize");
    assert!(size.contains("lCPI") && !size.contains("movk"), "{size}");
}
