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
