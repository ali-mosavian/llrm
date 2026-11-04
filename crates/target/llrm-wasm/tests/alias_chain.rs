use std::path::Path;
use std::process::Command;

/// `WebAssemblySubtarget::useAA` is true: the pin's chain walk asks alias
/// analysis, so a store waits for its own load only. Without it nbody_fixed
/// -O1 chained `vy`'s load behind `vx`'s store through both loads, loaded
/// both before the first store and spent a local on it (4 B over the pin).
#[test]
fn a_store_does_not_wait_for_a_load_of_another_global() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/alias_chain.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let accesses: Vec<&str> = asm
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("i32.load") || line.starts_with("i32.store"))
        .collect();
    let kinds: Vec<&str> = accesses.iter().map(|line| line.split_whitespace().next().unwrap()).collect();
    assert_eq!(kinds, ["i32.load", "i32.store", "i32.load", "i32.store"], "{asm}");
}
