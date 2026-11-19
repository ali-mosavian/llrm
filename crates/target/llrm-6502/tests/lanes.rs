//! The lane table of the 6502 register file: a lane per byte of each pair.

use llrm_codegen::backend::lanes::{Lanes, enter};
use llrm_lir::registers::{RegId, Regs};

fn id(name: &str) -> RegId {
    Regs(&llrm_6502::REGISTER_INFO).table.iter().flatten().find(|entry| entry.name == name).expect("a register").id
}

/// The scheduler's first lane query panicked ("DX bytes 0..=0 are no lanes"):
/// the file has views under four bytes, so it was given x86's lane table, in
/// which its ids are no registers. Every function refused to compile.
#[test]
fn the_bytes_of_a_pair_are_two_lanes_and_a_cell_is_one_of_them() {
    let _lanes = enter(Regs(&llrm_6502::REGISTER_INFO));
    let (low, high, pair) = (Lanes::span(id("rc2"), 0, 1), Lanes::span(id("rc3"), 0, 1), Lanes::span(id("rs1"), 0, 2));
    assert_ne!(low.mask(), high.mask());
    assert_eq!(pair.mask(), low.mask() | high.mask());
    assert_eq!(Lanes::span(id("rs1"), 1, 1).mask(), high.mask());
    assert_eq!(Lanes::span(id("a"), 0, 1).mask() & pair.mask(), 0);
}
