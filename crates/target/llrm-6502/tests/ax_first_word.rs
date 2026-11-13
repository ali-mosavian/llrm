//! The first word argument in A:X (llvm-mos's CC_MOS), staged behind
//! `abi::with_first_word_in_ax`: off, every word is in the cells as before.

mod common;

use common::pipeline::datalayout;
use llrm_6502::abi::with_first_word_in_ax;
use llrm_6502::driver::{Level, Request, assembly};

const IR: &str = "define i16 @add16(i16 %a, i16 %b) noinline {
  %r = add i16 %a, %b
  ret i16 %r
}
define i16 @caller(i16 %x) {
  %r = call i16 @add16(i16 %x, i16 7)
  ret i16 %r
}
";

fn text(on: bool) -> String {
    let ir = format!("target datalayout = \"{}\"\n{IR}", datalayout());
    with_first_word_in_ax(on, || assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}")))
}

fn function<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    text[from..].split(".segment").next().unwrap()
}

/// `add16` was 13 B (llvm-mos and the hand code 10): both words came from
/// cells, `lda rc4; adc rc2`. With A:X the first word is already in the
/// registers and only the second is read from `rc2`/`rc3`.
#[test]
fn the_first_word_arrives_in_a_and_x_and_the_second_in_the_first_pair() {
    let on = text(true);
    let add = function(&on, "add16");
    assert!(add.contains("adc rc2") && add.contains("rc3"), "{add}");
    assert!(!add.contains("rc5"), "{add}");
    let off = text(false);
    assert!(function(&off, "add16").contains("rc5"), "{off}");
}

/// The caller's own first word is already in A:X: it is handed on (a tail
/// call) and the 7 goes in `rc2:rc3`, with no `rc4`/`rc5` cell for either.
#[test]
fn a_call_hands_the_first_word_over_in_a_and_x() {
    let on = text(true);
    let caller = function(&on, "caller");
    assert!(caller.contains("add16"), "{caller}");
    assert!(caller.contains("sty rc2") && !caller.contains("rc4"), "{caller}");
}
