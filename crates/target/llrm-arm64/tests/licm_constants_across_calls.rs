//! A loop with a call hoisted three constants and left `mov w, #1000` inside:
//! the room for hoisting was the callee-saved registers less those held, where
//! `MachineLICM` counts the whole register class (recpow -O1: one extra
//! instruction per iteration, 76427 instructions against the pin's 76029).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_constant_read_once_in_a_loop_with_a_call_is_hoisted() {
    let asm = driver::assembly(include_str!("data/latch_branch.ll"), Platform::Linux, Level::O1).expect("lowers");
    let bench = asm.split("bench_recpow:").nth(1).and_then(|rest| rest.split("power:").next()).unwrap_or("");
    let at = |needle: &str| bench.find(needle);
    let (mov, call) = (at("#1000"), at("bl power").or_else(|| at("bl ")));
    assert!(matches!((mov, call), (Some(m), Some(c)) if m < c), "{asm}");
}
