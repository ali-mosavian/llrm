use std::path::Path;
use std::process::Command;

/// A select of two constants was an `i32.sub` of the condition, then a shift
/// once the index scaled it: three instructions in bintree's insertion loop,
/// where the pin selects between `4` and `8` (bintree -O2 2200 instructions
/// over the pin on the same IR).
#[test]
fn a_select_of_constants_stays_a_select() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/select_consts.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(asm.contains("i32.select") && !asm.contains("i32.sub") && !asm.contains("i32.shl"), "{asm}");
}
