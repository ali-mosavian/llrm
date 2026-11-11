use std::path::Path;
use std::process::Command;

/// The expander reused `fillw`'s `zext nneg i16 %n` and dropped the flag, so
/// the signext parameter was masked with `i32.const 65535` (pattern_fill -O2
/// was 26 B over).
#[test]
fn a_signext_parameter_known_positive_is_not_masked_when_widened() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/fill_signext.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(!asm.contains("65535"), "{asm}");
}
