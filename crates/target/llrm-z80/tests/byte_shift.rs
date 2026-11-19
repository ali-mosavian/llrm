//! DEVIATE Z80-BYTE-SHIFT: a word whose high byte is zero, shifted right by less than 8, is shifted as a byte.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

/// huffman_decode's `getbit`: `stream[i] >> (7 - n)` with the byte promoted to int. The shift ran on the word, a
/// `srl h; rr l` loop (`__lshrhi3`), two instructions a step; the byte loop is one.
const IR: &str = include_str!("data/byte_shift.ll");

#[test]
fn a_promoted_byte_shifted_by_less_than_eight_is_a_byte_shift() {
    for level in [Level::Os, Level::O2] {
        let request = Request::new(level);
        let prepared = prepare(IR, &request).expect("prepares");
        let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == "getbit").expect("getbit");
        let text = text_in(&prepared, name, number, &request).expect("compiles");
        assert!(text.contains("__lshrqi3"), "{level:?}: no byte shift:\n{text}");
        assert!(!text.contains("__lshrhi3"), "{level:?}: the word shift stayed:\n{text}");
    }
}

/// A mask that fits the byte a word was extended from: `zext(x) & 1` left a `ld c,a; ld b,#0; ld a,c` round trip
/// through a pair (+1 B in huffman_decode `getbit`).
#[test]
fn a_mask_within_an_extended_byte_is_a_byte_and() {
    for level in [Level::Os, Level::O2] {
        let request = Request::new(level);
        let prepared = prepare(IR, &request).expect("prepares");
        let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == "getbit").expect("getbit");
        let text = text_in(&prepared, name, number, &request).expect("compiles");
        assert!(!text.contains("ld b, #0"), "{level:?}: the byte went through a pair:\n{text}");
    }
}
