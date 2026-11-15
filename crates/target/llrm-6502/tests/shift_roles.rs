//! A word shifted in A:X keeps in `a` the byte that arrives and leaves there.
//! `x >> 3` was 16 B (`sta rc2; txa; ..; tax; lda rc2`) against 14, `x >> 7`
//! 15 against 12 and `x >> 4` (arithmetic) 27 against 17 (2026-11-15).

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

fn bytes(
    level: Level,
    op: &str,
    k: u32,
) -> (u32, String) {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i16 @f(i16 %x) {{\n  %r = {op} i16 %x, {k}\n  ret i16 %r\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("f:").unwrap_or_else(|| panic!("no `f:` in\n{text}"));
    let body: String = text[from..].split("  rts").next().unwrap().to_owned() + "  rts\n";
    (estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes, body)
}

fn at_most(
    op: &str,
    k: u32,
    limit: u32,
) {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let (got, body) = bytes(level, op, k);
        assert!(got <= limit, "{level:?} {op} {k}: {got} B against {limit}\n{body}");
    }
}

#[test]
fn a_word_shifted_right_by_three_keeps_the_low_byte_in_a() {
    at_most("lshr", 3, 14);
}

#[test]
fn a_word_shifted_right_by_seven_rotates_its_high_byte_once() {
    at_most("lshr", 7, 12);
}

#[test]
fn a_word_arithmetic_shift_right_by_four_is_a_counted_loop_in_a() {
    at_most("ashr", 4, 17);
}
