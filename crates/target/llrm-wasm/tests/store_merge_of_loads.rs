use std::path::Path;
use std::process::Command;

/// Three byte copies were three `load8_u`/`store8` pairs (stack_arrays_copy): the store merge of loads found no legal
/// width for `i16` because the pin's `TypePromoteInteger` branch of `tryStoreMergeOfLoads` was missing. The pin
/// (clang -O2 --target=wasm32) makes the last two bytes one `load16_u`/`store16`.
#[test]
fn three_byte_copies_are_a_byte_and_a_halfword_copy() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/copy3.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc")).args(["-S", "-O2", "-o", "-"]).arg(ir).output().expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("i32.load16_u") && text.contains("i32.store16"), "{text}");
    assert_eq!(text.matches("i32.load8_u").count(), 1, "{text}");
}
