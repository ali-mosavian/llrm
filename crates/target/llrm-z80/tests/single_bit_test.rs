//! `(x & (1 << n)) == 0` was a load, an `and`, a copy and an `or a, a` (a
//! long's bit 8: `ld c, #0; ld a, h; and a, #1; ld b, a; or a, a`, 7 B); it is
//! `bit n, r`, 2 B.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

const IR: &str = include_str!("data/single_bit_test.ll");

fn text(function: &str) -> String {
    let request = Request::new(Level::O1);
    let prepared = prepare(IR, &request).expect("prepares");
    let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == function).expect("function");
    text_in(&prepared, name, number, &request).expect("compiles")
}

#[test]
fn a_bit_of_a_long_is_a_bit_instruction() {
    let text = text("high_bit_of_a_long");
    assert!(text.contains("bit 0, h"), "{text}");
    assert!(!text.contains("and a"), "{text}");
}

#[test]
fn a_bit_of_a_byte_is_a_bit_instruction() {
    let text = text("low_bit_of_a_byte");
    assert!(text.contains("bit 4, "), "{text}");
}
