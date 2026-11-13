//! A word range check on a value below 256 is a byte range check and a zero high byte.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `c >= 9 && c <= 13` (`isspace`) subtracted 9 from both bytes of `c` and
/// compared both with 5 (`clc; adc #247; tax; lda rc3; adc #255; tay` ...):
/// 51 B, 81 clk. It is `hi == 0` and a byte subtract: no `#255` high-byte add.
#[test]
fn a_word_range_check_below_256_subtracts_one_byte() {
    let ir = "define i16 @r(i16 %c) nounwind {\n  %d = add i16 %c, -9\n  %b = icmp ult i16 %d, 5\n  %z = zext i1 %b to i16\n  ret i16 %z\n}\n";
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("r:").unwrap_or_else(|| panic!("{text}"));
    let body = text[from..].split(".segment").next().unwrap();
    assert!(!body.contains("adc #255"), "{body}");
}

/// A range that reaches past 256 is not below it: the word check stays.
#[test]
fn a_word_range_check_that_reaches_256_stays_a_word_check() {
    let ir = "define i16 @r(i16 %c) nounwind {\n  %d = add i16 %c, -250\n  %b = icmp ult i16 %d, 10\n  %z = zext i1 %b to i16\n  ret i16 %z\n}\n";
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("r:").unwrap_or_else(|| panic!("{text}"));
    let body = text[from..].split(".segment").next().unwrap();
    assert!(body.contains("adc #255"), "{body}");
}
