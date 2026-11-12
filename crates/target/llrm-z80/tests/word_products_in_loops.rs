//! The product of two zero-extended words in a loop was the `__mulsi3` call at -O2 when the module links it for its long
//! products: 58 B and 32 steps, half of them against zero (mul_words_to_long: 222 B, 204576 T). The high half of a
//! register pair built from two words is known (`BUILD_PAIR`), so the call is `__umulhisi3`: 143 B, 142854 T.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

const IR: &str = include_str!("data/word_products_in_loops.ll");

#[test]
fn a_word_product_in_a_loop_calls_the_word_routine() {
    let request = Request::new(Level::O2);
    let prepared = prepare(IR, &request).expect("prepares");
    let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == "products").expect("function");
    let text = text_in(&prepared, name, number, &request).expect("compiles");
    assert!(text.contains("__umulhisi3"), "{text}");
    assert!(!text.contains("__mulsi3"), "{text}");
}
