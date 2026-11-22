//! `MachineCSE`'s partial-redundancy hoist makes a value no instruction
//! defines: QuickrBasic `using.c` (68020 -O2, `usng_next_field`) panicked in
//! `machine_sink`, index 111 of 100 register classes.

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn a_hoisted_partially_redundant_address_compiles() {
    let source = include_str!("../data/pre_cse_hoist.ll");
    for cpu in ["68000", "68020"] {
        assembly(source, cpu_of(cpu).unwrap(), Level::O2).unwrap_or_else(|why| panic!("{cpu}: {why}"));
    }
}
