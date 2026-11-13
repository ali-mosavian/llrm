//! The byte table of the rate-priced decisions counts bytes: arm64's table was
//! the default of 1 per operation, so an unroll or an inline "added 42 B" was
//! 42 instructions (168 B) at a rate of 250 clocks a byte (nbody_single -O3:
//! the trial said -6 B, the kernel grew 460 B).

use llrm_arm64::machine::{Arm64Machine, Platform};
use llrm_mir::target::Machine;

#[test]
fn an_arm64_instruction_is_four_bytes_in_the_rate_priced_table() {
    let bytes = Arm64Machine { platform: Platform::Linux }.byte_costs();
    assert_eq!((bytes.add, bytes.load, bytes.store, bytes.branch, bytes.float_multiply), (4, 4, 4, 4, 4));
    assert_eq!(bytes.store_immediate, 8, "a constant store is a movz and the store");
}

#[test]
fn the_size_table_still_counts_instructions() {
    let units = Arm64Machine { platform: Platform::Linux }.size_costs();
    assert_eq!((units.add, units.load, units.store), (1, 1, 1));
}
