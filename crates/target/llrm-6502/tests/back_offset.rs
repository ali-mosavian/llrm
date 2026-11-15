//! `out - (hi << 8 | lo)` read from two bytes, as clang's `sub 0, hi` form.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

/// What clang -O2 leaves of `out - (in[0] | in[1] << 8)` (lz4's match source).
const BACK: &str = "define ptr @k_back(ptr %out, ptr %in) {
  %lo = load i8, ptr %in, align 1
  %lo16 = zext i8 %lo to i16
  %p1 = getelementptr inbounds i8, ptr %in, i16 1
  %hi = load i8, ptr %p1, align 1
  %neg = sub i8 0, %hi
  %neg16 = zext i8 %neg to i16
  %shifted = shl nuw i16 %neg16, 8
  %off = sub i16 %shifted, %lo16
  %src = getelementptr inbounds i8, ptr %out, i16 %off
  ret ptr %src
}
";

/// `0 - lo` and `0 - hi` were two borrow chains, added to `out` by a third:
/// 39 B where one `sbc` chain over the two loaded bytes is 26.
#[test]
fn a_negated_offset_of_two_bytes_is_one_subtract_chain() {
    let ir = format!("target datalayout = \"{}\"\n{BACK}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("k_back:").unwrap_or_else(|| panic!("no k_back in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    let adds = body.lines().filter(|line| line.trim_start().starts_with("adc")).count();
    assert!(adds == 0 && bytes <= 26, "{adds} adc, {bytes} B\n{body}");
}
