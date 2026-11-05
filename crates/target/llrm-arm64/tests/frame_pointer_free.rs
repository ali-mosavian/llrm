//! `x29` is an allocatable callee-saved register in a function with no frame
//! record (`AArch64RegisterInfo::getReservedRegs` reserves it only when
//! `hasFP`).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// The allocator reads the target and frame-pointer facts from process-wide
/// flags it sets per function, so two tests may not lower at once.
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A leaf holding thirty values across a loop spilled for want of a register
/// while `x29` sat idle (nbody_fixed -O2: 860 B, the pin's 688 B).
#[test]
fn a_leaf_under_pressure_uses_x29() {
    let _one_at_a_time = LOCK.lock().unwrap_or_else(|held| held.into_inner());
    let asm = driver::assembly(include_str!("data/frame_pointer_free.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("x29, [sp"), "x29 is saved and used: {asm}");
    assert!(!asm.contains("add x29, sp"), "no frame record: {asm}");
}

/// Darwin keeps `x29` as the frame pointer whatever the function (its ABI).
#[test]
fn darwin_never_allocates_x29() {
    let _one_at_a_time = LOCK.lock().unwrap_or_else(|held| held.into_inner());
    let asm =
        driver::assembly(include_str!("data/frame_pointer_darwin.ll"), Platform::Darwin, Level::O2).expect("lowers");
    let used: Vec<&str> = asm
        .lines()
        .filter(|line| line.contains("x29") && !line.contains("x29, x30") && !line.contains("add x29, sp"))
        .collect();
    assert!(used.is_empty(), "x29 stays the frame pointer: {used:?}");
}
