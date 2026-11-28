//! Two stores to one global at different offsets have disjoint memory
//! operands, so the first moves down past a pre-indexed store to the same
//! global and the two pair (the pin's `MachineMemOperand` carries the offset).
//! Without offsets `stp x8, x10, [x8, #48]` was two `str`s (lru -O3: one
//! instruction and 4 bytes over the pin).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn stores_to_different_offsets_of_a_global_pair_past_a_writeback_store() {
    let asm = driver::assembly(include_str!("data/store_pair_past_writeback.ll"), Platform::Linux, Level::O2)
        .expect("lowers");
    assert!(asm.lines().any(|line| line.trim().starts_with("stp x") && line.contains("#48]")), "{asm}");
}
