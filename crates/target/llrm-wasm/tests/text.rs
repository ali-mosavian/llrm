//! IR to assembly text through the whole backend against the text of `llc
//! -mtriple=wasm32 -O0 -fast-isel=false` (system LLVM 20.1.8,
//! `tests/golden/*.s`, one function each, of `tests/golden/four.ll`).

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::assembly;

fn golden(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/golden/{name}.s", env!("CARGO_MANIFEST_DIR"))).expect("golden file")
}

fn four(options: Options) -> String {
    let source = golden_source();
    assembly(&source, Width::W32, options).unwrap_or_else(|why| panic!("{why}"))
}

fn golden_source() -> String {
    std::fs::read_to_string(format!("{}/tests/golden/four.ll", env!("CARGO_MANIFEST_DIR"))).expect("four.ll")
}

/// Comments and blank lines are not compared (`compare.sh`'s `norm`).
fn norm(text: &str) -> String {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim_end())
        .filter(|line| !line.is_empty())
        .map(|line| format!("{line}\n"))
        .collect()
}

#[test]
fn the_four_functions_print_as_llc_does() {
    let wanted: String = ["ld", "st", "addr", "sum3"].iter().map(|name| golden(name)).collect();
    assert_eq!(norm(&four(Options { stackify: false })), norm(&wanted));
}

/// No golden (the pin's -O0 order has no oracle here): the stackified functions
/// keep a single-use value on the stack.
#[test]
fn the_stackified_functions_print() {
    let text = four(Options::default());
    if let Some(path) = std::env::var_os("WASM_DUMP_TEXT") {
        std::fs::write(path, &text).expect("dump");
    }
    assert!(!text.contains("local.set"), "the single-use values stay on the stack");
}
