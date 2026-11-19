use std::path::Path;
use std::process::Command;

/// An `i8` bit reverse was promoted to `i32` before it was expanded, so it
/// paid for a byte swap and 32-bit masks: stft_single -O2 ran 54 B over the
/// pin. The pin expands it at `i8` ("try to expand now"): masks 15, 51, 85.
#[test]
fn an_i8_bit_reverse_is_expanded_at_its_own_width() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/bitrev8.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(asm.contains("i32.const\t85"), "{asm}");
    assert!(!asm.contains("i32.bswap") && !asm.contains("i32.rotr"), "{asm}");
}
