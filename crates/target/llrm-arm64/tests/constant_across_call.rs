//! A constant the loop uses is not kept alive across the call after it.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// recpow -Os `power` was 92 B against gcc's 84: the prepass merged the
/// constant made for the loop with the one made after the recursive call, so
/// one definition was read after the call and sat in the callee-saved x20
/// (a 32-byte frame with a second saved pair). The pin shares a cheap constant
/// only with a block that follows its definition (MachineCSE.cpp
/// `isProfitableToCSE`, heuristic 1), so the call's constant is made again.
#[test]
fn a_loop_constant_is_made_again_after_a_call_not_kept_in_a_saved_register() {
    let asm = driver::assembly(include_str!("data/linear_recursion_peel.ll"), Platform::Linux, Level::Os).expect("lowers");
    let power: Vec<&str> = asm.lines().skip_while(|line| *line != "power:").skip(1).take_while(|line| !line.ends_with(':') || line.starts_with(".L")).collect();
    let saved = power.iter().filter(|line| line.contains("x20") || line.contains("w20")).count();
    assert_eq!(saved, 0, "the constant 30011 sits in a callee-saved register:\n{}", power.join("\n"));
}
