//! A pointer walked over two neighbouring accesses starts at the first, so the
//! pair loads and bumps the pointer in one instruction (`ldp s, s, [p], #44`).
//! The pin's immediate rating (`-4` is cheaper than `4`) started it at the
//! second: `ldp [p, #-4]` and a separate `add`, one instruction a trip more
//! than gcc (sfm16_float_mul -O1: 4,009 against 3,612).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_pair_of_loads_walks_from_its_first_access() {
    let asm = driver::assembly(include_str!("data/postinc_pair.ll"), Platform::Linux, Level::O1).expect("lowers");
    assert!(asm.lines().any(|line| line.trim_start().starts_with("ldp") && line.contains("], #44")), "{asm}");
}
