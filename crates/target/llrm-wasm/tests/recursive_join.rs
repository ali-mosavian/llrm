//! `deep`'s call result joins a phi, an `add` and the `ret`. TCE-ACC-PHI made
//! it an accumulator loop, which the runtime unroll tripled and, with one use,
//! the inliner took into its caller: `recursive_stack_args` at O2 was 388 B and
//! 197 instructions against the pin's 165 and 129. The bar on wasm is the pin
//! alone, which keeps the recursion.

use std::path::Path;
use std::process::Command;

fn asm(
    file: &str,
    level: &str,
) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir").join(file);
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", level, "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn a_recursive_call_through_a_join_stays_a_call_on_wasm() {
    for level in ["-O1", "-O2", "-Os"] {
        let out = asm("recursive_join.ll", level);
        assert!(out.contains("call\tdeep"), "{level}: the recursion was made a loop:\n{out}");
    }
}

/// The same recursion as an internal function with one caller, as a C `static`
/// is: the early module pass that makes a linear recursion a loop ran with no
/// target, so wasm's switch (`eliminates_accumulator_through_join`) was never
/// asked and `recursive_stack_args` stayed 388 B against the pin's 165.
#[test]
fn an_internal_recursion_through_a_join_stays_a_call_on_wasm() {
    for level in ["-O2", "-Os"] {
        let out = asm("recursive_join_private.ll", level);
        assert!(out.contains("call\tdeep"), "{level}: the recursion was made a loop:\n{out}");
    }
}
