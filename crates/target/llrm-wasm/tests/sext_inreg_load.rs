use std::path::Path;
use std::process::Command;

/// `(sext_inreg (extload x))` was not folded into a sign-extending load, so
/// a 16-bit sample read as `i32.load16_u; i32.extend16_s` (stft_single -O2:
/// 2 B over the pin, which emits `i32.load16_s`).
#[test]
fn a_signed_halfword_is_read_with_a_sign_extending_load() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/sext_inreg_load.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(asm.contains("i32.load16_s"), "{asm}");
    assert!(!asm.contains("i32.extend16_s"), "{asm}");
}
