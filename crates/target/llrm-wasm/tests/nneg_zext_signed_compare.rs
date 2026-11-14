use std::path::Path;
use std::process::Command;

/// rep_tails wasm -O1 ran 152 instructions over the pin: a `zext nneg` of a
/// loaded `i16` read by a signed compare stayed a `zextload` plus a compare
/// on the sign-extended value, so each use re-masked with `i32.and 65535`.
/// The pin's `tryToFoldExtOfLoad` (`NonNegZExt`) makes it a `sextload`.
#[test]
fn a_zext_nneg_of_a_load_with_a_signed_compare_is_a_sextload() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/nneg_zext_signed_compare.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(!asm.contains("i32.and"), "{asm}");
}
