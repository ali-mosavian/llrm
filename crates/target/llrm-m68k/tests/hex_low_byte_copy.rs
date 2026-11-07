//! `hex`'s digit byte was stored through a copy of its wide value into a second
//! register (`move.l %d1,%d3; move.b %d3,(7,%a0,%d0)`) with `d3` saved around
//! the function: C13 switched off every join between widths, including this
//! one, where the byte's range lies inside the wide value's. 4 B and 24 clk.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = include_str!("fixtures/hex_low_byte_copy.ll");

#[test]
fn a_low_byte_of_a_live_value_is_stored_from_that_value() {
    let out = assembly(IR, cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    assert!(out.contains("move.b\t%d1, (7,%a0,%d0)"), "{out}");
    assert!(!out.contains("%d3"), "{out}");
}
