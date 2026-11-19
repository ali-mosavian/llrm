use std::path::Path;
use std::process::Command;

/// `srem i16 -1, x == -1` was masked to 16 bits (`i32.and 65535` against 65535): the
/// remainder's sign bits (those of the dividend) were not known, so the promoted compare
/// was zero-extended. The pin compares the 32-bit remainder with -1 (div_constants_short
/// -O3 was 10 instructions larger).
#[test]
fn a_remainder_of_minus_one_is_compared_unmasked() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/srem_compare_sign_bits.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains("i32.and"), "{text}");
}
