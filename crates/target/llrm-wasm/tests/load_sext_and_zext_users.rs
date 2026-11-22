use std::path::Path;
use std::process::Command;

/// One `i16` load read by a `sext` and a `zext` came out as `load16_s` plus
/// `i32.and 65535` (+2 B on pattern_fill -O1): type legalization released the
/// users of a node oldest first, the pin walks the use list newest first, so
/// the `sext_inreg` was combined before the mask and made the sign-extending
/// load.
#[test]
fn a_load_with_a_sext_and_a_zext_user_is_one_zero_extending_load() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/load_sext_and_zext_users.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(asm.contains("i32.load16_u") && !asm.contains("i32.and"), "{asm}");
}
