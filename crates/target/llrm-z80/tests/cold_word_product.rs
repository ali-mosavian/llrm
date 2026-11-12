//! The product of two zero-extended words was partial products everywhere at -O2: 216 B in a block that runs once
//! (tail_call_long_argument: 258 B). A block outside every loop calls `__umulhisi3`; a loop keeps the products inline.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

const IR: &str = include_str!("data/cold_word_product.ll");

fn text(function: &str) -> String {
    let request = Request::new(Level::O2);
    let prepared = prepare(IR, &request).expect("prepares");
    let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == function).expect("function");
    text_in(&prepared, name, number, &request).expect("compiles")
}

#[test]
fn a_word_product_outside_a_loop_is_a_call() {
    assert!(text("once").contains("__umulhisi3"), "{}", text("once"));
}

#[test]
fn a_word_product_in_a_loop_stays_inline() {
    assert!(!text("looped").contains("__umulhisi3"), "{}", text("looped"));
}
