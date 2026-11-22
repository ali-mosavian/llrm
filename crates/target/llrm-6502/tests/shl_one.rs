//! `1 << n` of a word was the generic shift of a constant: `1` made in two
//! cells, the count in a third and a 16-clock-a-bit loop over the pair
//! (bit_setclear: 856 clocks against llvm-mos's 793). The bit is made in a
//! byte, 5 clocks a bit, by a routine that touches no cell.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

#[test]
fn a_word_bit_from_a_count_calls_the_routine_that_makes_it() {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i16 @bit(i16 %n) {{\n  %b = shl i16 1, %n\n  ret i16 %b\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("__shl1hi3") && !text.contains("__ashlhi3"), "{text}");
}
