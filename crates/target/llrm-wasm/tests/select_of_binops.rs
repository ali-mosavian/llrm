use std::path::Path;
use std::process::Command;

/// shellsort wasm -O2/-O3 ran 30 instructions over the pin: `select c, x-33,
/// x-1` kept both adds in the loop. The pin's `foldSelectOfBinops` selects
/// the constants and adds `x` once (5 adds in the loop, not 6).
#[test]
fn a_select_of_two_adds_of_one_value_adds_once() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/select_of_adds.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = String::from_utf8_lossy(&out.stdout);
    assert_eq!(asm.matches("i32.add").count(), 5, "{asm}");
}
