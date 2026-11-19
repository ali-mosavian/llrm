use std::path::Path;
use std::process::Command;

fn asm() -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/exact_shift.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `shl (lshr exact x, 1), 2` with a second reader of the `lshr` shifted the
/// kept value and held it in a local (`local.tee 0` .. `i32.const 2; i32.shl`;
/// stft_single -O1, +6 B). The pin's `visitSHL` exact fold shifts the source:
/// `i32.const 1; i32.shl`.
#[test]
fn a_shl_of_an_exact_lshr_shifts_the_source() {
    let asm = asm();
    assert!(!asm.contains("local.tee"), "the shifted value is kept in a local:\n{asm}");
}
