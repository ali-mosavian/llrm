use std::path::Path;
use std::process::Command;

fn asm() -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/frozen_ext_load.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `zext (freeze (load i8))` stayed a narrow load plus `i32.and 255`, and the
/// `sext` a `load8_u` plus `i32.extend8_s` (bench_jpeg_fixed -O1, +2 B per
/// site). The pin's `tryToFoldExtOfLoad` (`Frozen`) makes each the extending
/// load.
#[test]
fn an_extend_of_a_frozen_load_is_the_extending_load() {
    let asm = asm();
    assert!(asm.contains("i32.load8_s"), "sext is a signed load:\n{asm}");
    assert!(!asm.contains("i32.and") && !asm.contains("extend8_s"), "no mask or re-extend:\n{asm}");
}
