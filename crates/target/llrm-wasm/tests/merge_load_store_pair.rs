use std::path::Path;
use std::process::Command;

/// Three byte copies at offsets 0..2 stayed three `load8_u`/`store8` pairs; the pin
/// merges two of them into one `i32.load16_u`/`i32.store16` (a truncstore of an
/// extload, `tryStoreMergeOfLoads`): stack_arrays_copy -O2 was 8 B larger.
#[test]
fn two_of_three_byte_copies_are_one_halfword_copy() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/merge_load_store_pair.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    let count = |needle: &str| text.lines().filter(|line| line.trim_start().starts_with(needle)).count();
    assert_eq!((count("i32.load16_u"), count("i32.store16")), (1, 1), "{text}");
    assert_eq!((count("i32.load8_u"), count("i32.store8")), (1, 1), "{text}");
}
