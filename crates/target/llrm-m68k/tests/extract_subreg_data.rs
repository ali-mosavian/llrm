//! A byte store of `c ? x + 87 : x + 48` read the select's merged value through
//! a plain copy, so the allocator could give it an address register and every
//! use paid `move.l %d2,%a1; lea (48,%a1),%a1; move.l %a1,%d2` (hex 68020 -Os
//! 90 B against gcc's 62). The pin's `ConstrainForSubReg` narrows the source of
//! a sub-register extract to a data register. `hex_raw.ll` is the bench kernel
//! `library/hex.ll` as the mid-end leaves it.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const HEX: &str = include_str!("fixtures/hex_raw.ll");

#[test]
fn a_value_stored_as_a_byte_does_not_bounce_through_an_address_register() {
    for cpu in ["68020", "68000"] {
        for level in [Level::O2, Level::Os] {
            let out = assembly(HEX, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(!out.contains("lea\t(48,"), "{cpu}: the arm is a lea into an address register:\n{out}");
        }
    }
}
