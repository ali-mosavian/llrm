//! `v = (v & ~7) | 5` on a byte of a bit field went `ld c, a` then `ld a, c`
//! between the `and` and the `or` (volatile_bitfield: 7 sites; div_constants_long: 36).

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_byte_just_stored_from_a_is_not_loaded_back_into_it() {
    let ir = include_str!("kernels/ir/bit_field_assign.ll");
    for level in [Level::O2, Level::Os] {
        let out = text(ir, "set", 0, &Request::new(level)).unwrap_or_else(|why| panic!("{why}"));
        let lines: Vec<&str> = out.lines().map(str::trim).collect();
        let trips = lines
            .windows(2)
            .filter(|pair| {
                pair[0].strip_prefix("ld ").and_then(|rest| rest.strip_suffix(", a")).is_some_and(|reg| pair[1] == format!("ld a, {reg}"))
            })
            .count();
        assert_eq!(trips, 0, "{trips} round trips:\n{out}");
    }
}
