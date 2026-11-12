//! A word times a constant that is not `2^k +- 1` was the `__mulhi3` call at -O2: `x * 100` was 6 B and a loop of
//! about 65 T a bit of the constant (variadic: 385 B, 4047 T). It is shifts and adds where the chain is cheaper.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

const IR: &str = include_str!("data/word_product_chain.ll");

fn text(function: &str) -> String {
    let request = Request::new(Level::O2);
    let prepared = prepare(IR, &request).expect("prepares");
    let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == function).expect("function");
    text_in(&prepared, name, number, &request).expect("compiles")
}

#[test]
fn a_word_times_a_constant_is_shifts_and_adds() {
    let text = text("by_hundred");
    assert!(!text.contains("__mulhi3"), "{text}");
    assert!(text.lines().count() < 30, "{text}");
}

#[test]
fn a_word_times_a_variable_is_the_call() {
    assert!(text("by_variable").contains("__mulhi3"));
}
