//! `x >> 7` of a word and of a long, end to end: the top bit is taken from the
//! carry the shift already left, and the shifted copy of the high byte is the
//! high byte itself. `k_shr7` was 11 instructions, `k_shr7l` 37 (2026-11-08).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn instructions(
    ir: &str,
    name: &str,
) -> Vec<String> {
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    text[from..]
        .split(".segment")
        .next()
        .unwrap()
        .lines()
        .skip(1)
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

#[test]
fn a_word_shifted_right_by_seven_shifts_its_bytes_in_place() {
    let lines = instructions("define i16 @k_shr7(i16 %x) {\n  %r = lshr i16 %x, 7\n  ret i16 %r\n}\n", "k_shr7");
    assert_eq!(
        lines,
        ["asl rc2", "rol rc3", "lda #0", "rol a", "tax", "lda rc3", "rts"],
        "the shift is two rotates in place and the carry is the top bit"
    );
}

#[test]
fn a_long_shifted_right_by_seven_is_no_longer_than_it_was() {
    let lines = instructions("define i32 @k_shr7l(i32 %x) {\n  %r = lshr i32 %x, 7\n  ret i32 %r\n}\n", "k_shr7l");
    assert!(lines.len() <= 35, "{} instructions: {lines:?}", lines.len());
}
