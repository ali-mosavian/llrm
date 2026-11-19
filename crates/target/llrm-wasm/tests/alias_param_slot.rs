use std::path::Path;
use std::process::Command;

/// wasm `memory_expansion` lr9 (mid-end and backend, -O2): the slots are passed to later calls, so alias
/// analysis called them captured and "may alias" a pointer parameter; the
/// copy stores stayed on the loads' chains and four byte copies were not one
/// word copy (18 loads and stores where the pin has 9).
#[test]
fn byte_copies_beside_a_captured_slot_merge_into_word_copies() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/alias_param_slot.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let accesses = asm
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("i32.load") || line.starts_with("i32.store"))
        .count();
    assert_eq!(accesses, 9, "{asm}");
}
