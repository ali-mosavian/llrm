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

const CREDIT: &str = include_str!("data/word_chains_credit.ll");

fn strict(ir: &str, function: &str) -> String {
    let request = Request::new(Level::O1);
    let prepared = prepare(ir, &request).expect("prepares");
    let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == function).expect("function");
    text_in(&prepared, name, number, &request).expect("compiles")
}

/// -O1 took the call for `x * 100` and `x * 11` (variadic +23 B: the 24 B routine linked for two sites): with no
/// other word multiply in the module the chains are smaller than the routine and the calls.
#[test]
fn a_module_of_constant_word_products_links_no_routine_at_o1() {
    for function in ["by_hundred", "by_eleven"] {
        let text = strict(CREDIT, function);
        assert!(!text.contains("__mulhi3"), "{text}");
    }
}

/// The routine is linked for the module's variable product, so the call costs 6 B and a loop of about 65 T a bit of the
/// constant (about 495 T for 100); -O1 judges by clocks too (2026-11-14 ruling) and pays a byte at 40 T: `x * 100` is the
/// chain.
#[test]
fn a_constant_word_product_is_the_chain_at_o1_where_the_clocks_repay_the_bytes() {
    assert!(!strict(IR, "by_hundred").contains("__mulhi3"));
}

/// -Os and -Oz paid no byte for clocks: `x * 1000` stayed the call, 6 B and about 1000 T against a chain of 12 B
/// (word_sign_zero_guard -Os: 371 B / 51701 T, +59.4% T over SDCC's 32430 T with 90 B to spare). A byte at 40 T, as at
/// -O1, takes the chain: 383 B / 29404 T.
#[test]
fn a_constant_word_product_is_the_chain_at_os_and_oz_where_the_clocks_repay_the_bytes() {
    for level in [Level::Os, Level::Oz] {
        let request = Request::new(level);
        let prepared = prepare(IR, &request).expect("prepares");
        let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == "by_hundred").expect("function");
        let text = text_in(&prepared, name, number, &request).expect("compiles");
        assert!(!text.contains("__mulhi3"), "{level:?}: {text}");
    }
}
