//! A copy that puts a value back where it already is is dropped.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// The argument was copied into the callee-saved `x19` and straight back into
/// `x0` for the call: one `mov` too many in every recursion of recpow's
/// `power` (the pin's MachineCopyPropagation drops it).
#[test]
fn a_value_copied_out_and_straight_back_is_copied_once() {
    let asm = driver::assembly(include_str!("data/shrink_wrap_loop.ll"), Platform::Linux, Level::O2).expect("lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let out = lines.iter().position(|line| *line == "orr x19, xzr, x0, lsl #0").expect("the copy out");
    assert_ne!(lines[out + 1], "orr x0, xzr, x19, lsl #0", "{asm}");
}
