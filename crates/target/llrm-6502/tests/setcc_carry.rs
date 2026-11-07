//! A compare of two words is one chain: the borrow of the low words into the high words' subtract.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

/// One compare of two `i32`, `k_<order>`, read through pointers.
fn compare(order: &str) -> String {
    format!(
        "define i16 @k_{order}(ptr %a, ptr %b) {{
  %x = load i32, ptr %a
  %y = load i32, ptr %b
  %c = icmp {order} i32 %x, %y
  %r = zext i1 %c to i16
  ret i16 %r
}}
"
    )
}

fn bytes_of(order: &str, limit: u32) {
    let name = format!("k_{order}");
    let ir = format!("target datalayout = \"{}\"\n{}", datalayout(), compare(order));
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= limit, "{name}: {bytes} B\n{body}");
}

/// The compare was a select of the compares of its two halves: three chains
/// and a 0/1 byte each, over 120 B for what one `sec; sbc` chain of four limbs is.
#[test]
fn an_unsigned_less_than_of_two_words_is_one_borrow_chain() {
    bytes_of("ult", 80);
}

#[test]
fn an_unsigned_less_or_equal_of_two_words_is_one_borrow_chain() {
    bytes_of("ule", 90);
}

#[test]
fn a_signed_less_than_of_two_words_is_one_borrow_chain() {
    bytes_of("slt", 90);
}

#[test]
fn a_signed_less_or_equal_of_two_words_is_one_borrow_chain() {
    bytes_of("sle", 106);
}
