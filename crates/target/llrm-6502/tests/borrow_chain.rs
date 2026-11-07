//! A multiword subtract written as halves is one borrow chain.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

/// The 64-bit subtract of `udivmod_w`'s `sub(n, d)`: `lo = a.lo - b.lo`,
/// `hi = a.hi - b.hi + sext(a.lo < b.lo)`, as the pin's `opt` leaves it.
const BORROW: &str = "define void @k_borrow(ptr %a, ptr %b, ptr %o) {
  %alo = load i32, ptr %a
  %pa = getelementptr i32, ptr %a, i16 1
  %ahi = load i32, ptr %pa
  %blo = load i32, ptr %b
  %pb = getelementptr i32, ptr %b, i16 1
  %bhi = load i32, ptr %pb
  %lt = icmp ult i32 %alo, %blo
  %lo = sub i32 %alo, %blo
  %b1 = sext i1 %lt to i32
  %h = sub i32 %ahi, %bhi
  %hi = add i32 %h, %b1
  store i32 %lo, ptr %o
  %po = getelementptr i32, ptr %o, i16 1
  store i32 %hi, ptr %po
  ret void
}
";

/// The borrow of the low halves was made as its own compare chain and a 0/255
/// byte, then added to the high halves with a second `adc` chain: 8 bytes of
/// `sbc` became three chains and a mask.
#[test]
fn the_borrow_of_a_split_subtract_is_the_carry_of_its_subtract() {
    let ir = format!("target datalayout = \"{}\"\n{BORROW}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("k_borrow:").unwrap_or_else(|| panic!("no k_borrow in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    let compares = body.lines().filter(|line| line.trim_start().starts_with("cmp ")).count();
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert_eq!(compares, 0, "{compares} compares, {bytes} B\n{body}");
}
