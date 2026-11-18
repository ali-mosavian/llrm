//! A word shift left by 6 or 7 of an extended byte ran a loop of `asl; rol`
//! passes over two bytes: fill_nest -Os spent 105 clocks a row on `y << 7`
//! (2026-11-12).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn body(shift: u32) -> String {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine void @f(ptr %p, i8 %b) {{\n  %w = zext i8 %b to i16\n  %s = shl nuw nsw i16 %w, {shift}\n  %q = getelementptr inbounds nuw i8, ptr %p, i16 %s\n  store i8 1, ptr %q\n  ret void\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    text.split(".segment").next().unwrap().to_owned()
}

#[test]
fn a_byte_shifted_left_by_5_to_7_in_a_word_makes_no_loop() {
    for shift in [6, 7] {
        let text = body(shift);
        assert!(!text.contains("bne"), "<< {shift}: {text}");
    }
}
