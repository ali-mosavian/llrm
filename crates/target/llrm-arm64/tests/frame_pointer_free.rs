//! `x29` is an allocatable callee-saved register in a function with no frame
//! record (`AArch64RegisterInfo::getReservedRegs` reserves it only when
//! `hasFP`).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// A leaf holding thirty values across a loop spilled for want of a register
/// while `x29` sat idle (nbody_fixed -O2: 860 B, the pin's 688 B).
#[test]
fn a_leaf_under_pressure_uses_x29() {
    let asm = driver::assembly(include_str!("data/frame_pointer_free.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("x29, [sp"), "x29 is saved and used: {asm}");
    assert!(!asm.contains("add x29, sp"), "no frame record: {asm}");
}
