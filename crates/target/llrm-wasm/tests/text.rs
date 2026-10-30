//! Selected DAG to assembly text: the whole chain of this crate against the
//! text of `llc -mtriple=wasm32 -O0 -fast-isel=false` (system LLVM 20.1.8,
//! `tests/golden/*.s`, one function each).
//!
//! The nodes are scheduled in the order of a walk from the root; the pin's
//! scheduler is not bound to a wasm DAG yet.

mod common;

use llrm_wasm::print;

fn golden(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/golden/{name}.s", env!("CARGO_MANIFEST_DIR"))).expect("golden file")
}

#[test]
fn the_four_functions_print_as_llc_does() {
    for f in common::four() {
        assert_eq!(print::function(&f), golden(&f.name), "{}", f.name);
    }
}

/// No golden (the pin's -O0 order has no oracle here): the text of the
/// stackified functions, dumped for reading.
#[test]
fn the_stackified_functions_print() {
    let text: String = common::four_with(true).iter().map(print::function).collect();
    if let Some(path) = std::env::var_os("WASM_DUMP_TEXT") {
        std::fs::write(path, &text).expect("dump");
    }
    assert!(!text.contains("local.set"), "the single-use values stay on the stack");
}
