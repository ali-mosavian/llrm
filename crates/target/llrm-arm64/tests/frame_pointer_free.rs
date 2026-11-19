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

/// With `x29` free the leaf still ran out of registers: `x16` and `x17` were
/// the lowerer's, reserved for the offsets a large frame cannot encode.
#[test]
fn a_small_frame_gives_x16_and_x17_to_the_allocator() {
    let _one_at_a_time = LOCK.lock().unwrap_or_else(|held| held.into_inner());
    let asm = driver::assembly(include_str!("data/frame_pointer_free.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("x16") && asm.contains("x17"), "{asm}");
}

/// A frame of 24 KB needs the pair for its offsets: they stay out of the
/// allocation (every use is an address built from `sp`).
#[test]
fn a_large_frame_keeps_x16_and_x17_for_its_offsets() {
    let _one_at_a_time = LOCK.lock().unwrap_or_else(|held| held.into_inner());
    let asm = driver::assembly(include_str!("data/frame_scratch_big.ll"), Platform::Linux, Level::O2).expect("lowers");
    let data_uses: Vec<&str> = asm
        .lines()
        .filter(|line| {
            (line.contains("x16") || line.contains("x17"))
                && !(line.contains("sp") || line.contains("movz") || line.contains("movk") || line.contains("[x1"))
        })
        .collect();
    assert!(data_uses.is_empty(), "{data_uses:?}");
}

/// `x16` and `x17` are clobbered by the linker's veneers at every call: a value
/// that lives across one never sits in them.
#[test]
fn nothing_live_across_a_call_is_in_x16_or_x17() {
    let _one_at_a_time = LOCK.lock().unwrap_or_else(|held| held.into_inner());
    let asm = driver::assembly(include_str!("data/live_across_call.ll"), Platform::Linux, Level::O2).expect("lowers");
    let (before, after) = asm.split_once("\tbl f").expect("the call");
    let written = |text: &str, reg: &str| {
        text.lines().any(|line| {
            let mut parts = line.trim().splitn(2, ' ');
            parts.next();
            parts.next().is_some_and(|ops| ops.split(',').next().is_some_and(|dst| dst.trim() == reg))
        })
    };
    for reg in ["x16", "x17"] {
        // Read after the call, before any write after it.
        let mut stale = false;
        for line in after.lines() {
            let line = line.trim();
            let ops = line.split_once(' ').map_or("", |(_, ops)| ops);
            let mut fields = ops.split(',').map(str::trim);
            let dst = fields.next().unwrap_or("");
            let reads = fields.any(|field| field == reg || field.contains(&format!("[{reg}")));
            if reads && !stale {
                stale = written(before, reg);
                break;
            }
            if dst == reg {
                break;
            }
        }
        assert!(!stale, "{reg} is read after the call but written before it: {asm}");
    }
}

/// The allocation made while the frame looked small (x16/x17 free) is thrown
/// away when the real frame is 24 KB: with it kept, a spill at a large offset
/// would clobber a value held in x16/x17.
#[test]
fn a_frame_that_outgrows_the_estimate_is_allocated_again_without_the_pair() {
    let _one_at_a_time = LOCK.lock().unwrap_or_else(|held| held.into_inner());
    llrm_arm64::frame::UNSURE.store(true, std::sync::atomic::Ordering::Relaxed);
    let asm = driver::assembly(include_str!("data/frame_scratch_big.ll"), Platform::Linux, Level::O2);
    llrm_arm64::frame::UNSURE.store(false, std::sync::atomic::Ordering::Relaxed);
    let asm = asm.expect("lowers");
    let data_uses: Vec<&str> = asm
        .lines()
        .filter(|line| {
            (line.contains("x16") || line.contains("x17"))
                && !(line.contains("sp") || line.contains("movz") || line.contains("movk") || line.contains("[x1"))
        })
        .collect();
    assert!(data_uses.is_empty(), "{data_uses:?}");
    assert!(asm.contains("x16") || asm.contains("x17"), "large offsets still use the pair: {asm}");
}
