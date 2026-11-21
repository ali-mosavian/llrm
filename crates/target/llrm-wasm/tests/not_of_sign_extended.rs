use std::path::Path;
use std::process::Command;

/// os_execute wasm32 was 5 B over the pin: `short h = open(..); report(h >= 0)`
/// is `~x >>> 31` of the sign-extended `x`. `srl (xor x, -1), 31` was turned
/// into `(x >>> 31) ^ 1`, which let `srl (sra (shl y, 16), 16), 31` reach
/// through the `sra` to `shl y, 16`, so the code shifted and sign-extended the
/// same value twice. The pin keeps a `not` (`visitShiftByConstant`).
#[test]
fn a_shift_of_a_not_keeps_the_not_and_one_sign_extension() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/not_of_sign_extended.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(!asm.contains("i32.shl"), "the sign bit is read through a shift left:\n{asm}");
    assert_eq!(asm.matches("i32.extend16_s").count(), 1, "{asm}");
}
